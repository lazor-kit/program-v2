import {
  Connection,
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  TransactionInstruction,
} from '@solana/web3.js';
import { PROGRAM_ID } from '../generated';
import { AuthorityAccount } from '../generated/accounts';
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda, findDeferredExecPda } from './pdas';
import { readAuthorityCounter } from './secp256r1';
import { packCompactInstructions, computeAccountsHash, computeInstructionsHash, type CompactInstruction } from './packing';
import {
  createCreateWalletIx,
  createAddAuthorityIx,
  createRemoveAuthorityIx,
  createTransferOwnershipIx,
  createExecuteIx,
  createCreateSessionIx,
  createAuthorizeIx,
  createExecuteDeferredIx,
  createReclaimDeferredIx,
  createRevokeSessionIx,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  DISC_ADD_AUTHORITY,
  DISC_REMOVE_AUTHORITY,
  DISC_TRANSFER_OWNERSHIP,
  DISC_EXECUTE,
  DISC_CREATE_SESSION,
  DISC_AUTHORIZE,
  DISC_REVOKE_SESSION,
} from './instructions';
import { signWithSecp256r1, buildDataPayloadForAdd, buildDataPayloadForTransfer, buildDataPayloadForSession, concatParts } from './signing';
import { buildCompactLayout } from './compact';
import type { CreateWalletOwner, AdminSigner, ExecuteSigner, Secp256r1SignerConfig, DeferredPayload } from './types';

// ─── Sysvar instruction indexes (auto-computed from account layouts) ──

const SYSVAR_IX_INDEX_ADD_AUTHORITY = 6;
const SYSVAR_IX_INDEX_REMOVE_AUTHORITY = 5;
const SYSVAR_IX_INDEX_TRANSFER_OWNERSHIP = 6;
const SYSVAR_IX_INDEX_EXECUTE = 4;
const SYSVAR_IX_INDEX_CREATE_SESSION = 6;
const SYSVAR_IX_INDEX_AUTHORIZE = 6;
const SYSVAR_IX_INDEX_REVOKE_SESSION = 5;

// ─── Internal helpers ─────────────────────────────────────────────────

/** Resolves a CreateWalletOwner to the low-level fields needed by IX builders */
function resolveOwnerFields(owner: CreateWalletOwner): {
  authType: number;
  credentialOrPubkey: Uint8Array;
  secp256r1Pubkey?: Uint8Array;
  rpId?: string;
} {
  if (owner.type === 'ed25519') {
    return { authType: AUTH_TYPE_ED25519, credentialOrPubkey: owner.publicKey.toBytes() };
  }
  return {
    authType: AUTH_TYPE_SECP256R1,
    credentialOrPubkey: owner.credentialIdHash,
    secp256r1Pubkey: owner.compressedPubkey,
    rpId: owner.rpId,
  };
}

/** Gets the credential bytes (PDA seed) from a CreateWalletOwner */
function ownerCredentialBytes(owner: CreateWalletOwner): Uint8Array {
  return owner.type === 'ed25519' ? owner.publicKey.toBytes() : owner.credentialIdHash;
}

export class LazorKitClient {
  constructor(
    public readonly connection: Connection,
    public readonly programId: PublicKey = PROGRAM_ID,
  ) {}

  // ─── PDA helpers ─────────────────────────────────────────────────

  findWallet(userSeed: Uint8Array) { return findWalletPda(userSeed, this.programId); }
  findVault(walletPda: PublicKey) { return findVaultPda(walletPda, this.programId); }
  findAuthority(walletPda: PublicKey, credIdHash: Uint8Array) {
    return findAuthorityPda(walletPda, credIdHash, this.programId);
  }
  findSession(walletPda: PublicKey, sessionKey: Uint8Array) {
    return findSessionPda(walletPda, sessionKey, this.programId);
  }
  findDeferredExec(walletPda: PublicKey, authorityPda: PublicKey, counter: number) {
    return findDeferredExecPda(walletPda, authorityPda, counter, this.programId);
  }

  // ─── Account readers ─────────────────────────────────────────────

  async fetchAuthority(authorityPda: PublicKey): Promise<AuthorityAccount> {
    return AuthorityAccount.fromAccountAddress(this.connection, authorityPda);
  }

  async readCounter(authorityPda: PublicKey): Promise<number> {
    return readAuthorityCounter(this.connection, authorityPda);
  }

  // ─── CreateWallet ────────────────────────────────────────────────

  /**
   * Create a new LazorKit wallet with the given owner.
   *
   * @example Ed25519 owner
   * ```typescript
   * const { instructions, walletPda, vaultPda } = client.createWallet({
   *   payer: payer.publicKey,
   *   userSeed: randomBytes(32),
   *   owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
   * });
   * ```
   *
   * @example Secp256r1 (passkey) owner
   * ```typescript
   * const { instructions, walletPda, vaultPda } = client.createWallet({
   *   payer: payer.publicKey,
   *   userSeed: randomBytes(32),
   *   owner: {
   *     type: 'secp256r1',
   *     credentialIdHash,
   *     compressedPubkey,
   *     rpId: 'example.com',
   *   },
   * });
   * ```
   */
  createWallet(params: {
    payer: PublicKey;
    userSeed: Uint8Array;
    owner: CreateWalletOwner;
  }): { instructions: TransactionInstruction[]; walletPda: PublicKey; vaultPda: PublicKey; authorityPda: PublicKey } {
    const [walletPda] = this.findWallet(params.userSeed);
    const [vaultPda] = this.findVault(walletPda);
    const { authType, credentialOrPubkey, secp256r1Pubkey, rpId } = resolveOwnerFields(params.owner);
    const [authorityPda, authBump] = this.findAuthority(walletPda, credentialOrPubkey);

    const ix = createCreateWalletIx({
      payer: params.payer, walletPda, vaultPda, authorityPda,
      userSeed: params.userSeed, authType, authBump,
      credentialOrPubkey, secp256r1Pubkey, rpId,
      programId: this.programId,
    });
    return { instructions: [ix], walletPda, vaultPda, authorityPda };
  }

  // ─── AddAuthority (unified) ─────────────────────────────────────

  /**
   * Add a new authority to the wallet.
   *
   * @example Add Ed25519 admin via Ed25519 owner
   * ```typescript
   * const { instructions, newAuthorityPda } = await client.addAuthority({
   *   payer: payer.publicKey,
   *   walletPda,
   *   adminSigner: ed25519(ownerKp.publicKey),
   *   newAuthority: { type: 'ed25519', publicKey: adminKp.publicKey },
   *   role: ROLE_ADMIN,
   * });
   * ```
   *
   * @example Add Secp256r1 spender via Secp256r1 owner
   * ```typescript
   * const { instructions, newAuthorityPda } = await client.addAuthority({
   *   payer: payer.publicKey,
   *   walletPda,
   *   adminSigner: secp256r1(ceoSigner),
   *   newAuthority: { type: 'secp256r1', credentialIdHash, compressedPubkey, rpId },
   *   role: ROLE_SPENDER,
   * });
   * ```
   */
  async addAuthority(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    adminSigner: AdminSigner;
    newAuthority: CreateWalletOwner;
    role: number;
  }): Promise<{ instructions: TransactionInstruction[]; newAuthorityPda: PublicKey }> {
    const { authType: newType, credentialOrPubkey, secp256r1Pubkey, rpId } = resolveOwnerFields(params.newAuthority);
    const [newAuthorityPda] = this.findAuthority(params.walletPda, credentialOrPubkey);
    const s = params.adminSigner;

    if (s.type === 'ed25519') {
      const ix = createAddAuthorityIx({
        payer: params.payer, walletPda: params.walletPda,
        adminAuthorityPda: s.authorityPda ?? this.findAuthority(params.walletPda, s.publicKey.toBytes())[0],
        newAuthorityPda, newType, newRole: params.role,
        credentialOrPubkey, secp256r1Pubkey, rpId,
        authorizerSigner: s.publicKey, programId: this.programId,
      });
      return { instructions: [ix], newAuthorityPda };
    }

    // Secp256r1
    const adminAuthorityPda = s.authorityPda
      ?? this.findAuthority(params.walletPda, s.signer.credentialIdHash)[0];
    const slot = s.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(adminAuthorityPda)) + 1;

    const dataPayload = buildDataPayloadForAdd(
      newType, params.role, credentialOrPubkey, secp256r1Pubkey, rpId,
    );
    const signedPayload = concatParts([dataPayload, params.payer.toBytes()]);

    const { authPayload, precompileIx } = await signWithSecp256r1({
      signer: s.signer, discriminator: new Uint8Array([DISC_ADD_AUTHORITY]),
      signedPayload, sysvarIxIndex: SYSVAR_IX_INDEX_ADD_AUTHORITY,
      slot, counter, payer: params.payer, programId: this.programId,
    });

    const ix = createAddAuthorityIx({
      payer: params.payer, walletPda: params.walletPda, adminAuthorityPda, newAuthorityPda,
      newType, newRole: params.role, credentialOrPubkey, secp256r1Pubkey, rpId,
      authPayload, programId: this.programId,
    });
    return { instructions: [precompileIx, ix], newAuthorityPda };
  }

  // ─── RemoveAuthority (unified) ──────────────────────────────────

  async removeAuthority(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    adminSigner: AdminSigner;
    targetAuthorityPda: PublicKey;
    refundDestination?: PublicKey;
  }): Promise<{ instructions: TransactionInstruction[] }> {
    const refundDest = params.refundDestination ?? params.payer;
    const s = params.adminSigner;

    if (s.type === 'ed25519') {
      const ix = createRemoveAuthorityIx({
        payer: params.payer, walletPda: params.walletPda,
        adminAuthorityPda: s.authorityPda ?? this.findAuthority(params.walletPda, s.publicKey.toBytes())[0],
        targetAuthorityPda: params.targetAuthorityPda, refundDestination: refundDest,
        authorizerSigner: s.publicKey, programId: this.programId,
      });
      return { instructions: [ix] };
    }

    const adminAuthorityPda = s.authorityPda
      ?? this.findAuthority(params.walletPda, s.signer.credentialIdHash)[0];
    const slot = s.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(adminAuthorityPda)) + 1;

    const signedPayload = concatParts([
      params.targetAuthorityPda.toBytes(), refundDest.toBytes(),
    ]);

    const { authPayload, precompileIx } = await signWithSecp256r1({
      signer: s.signer, discriminator: new Uint8Array([DISC_REMOVE_AUTHORITY]),
      signedPayload, sysvarIxIndex: SYSVAR_IX_INDEX_REMOVE_AUTHORITY,
      slot, counter, payer: params.payer, programId: this.programId,
    });

    const ix = createRemoveAuthorityIx({
      payer: params.payer, walletPda: params.walletPda, adminAuthorityPda,
      targetAuthorityPda: params.targetAuthorityPda, refundDestination: refundDest,
      authPayload, programId: this.programId,
    });
    return { instructions: [precompileIx, ix] };
  }

  // ─── TransferOwnership (unified) ────────────────────────────────

  /**
   * Transfer wallet ownership to a new authority.
   *
   * @example Transfer to new Secp256r1 owner
   * ```typescript
   * const { instructions } = await client.transferOwnership({
   *   payer: payer.publicKey,
   *   walletPda,
   *   ownerSigner: secp256r1(ceoSigner),
   *   newOwner: { type: 'secp256r1', credentialIdHash, compressedPubkey, rpId },
   * });
   * ```
   */
  async transferOwnership(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    ownerSigner: AdminSigner;
    newOwner: CreateWalletOwner;
  }): Promise<{ instructions: TransactionInstruction[]; newOwnerAuthorityPda: PublicKey }> {
    const { authType: newType, credentialOrPubkey, secp256r1Pubkey, rpId } = resolveOwnerFields(params.newOwner);
    const [newOwnerAuthorityPda] = this.findAuthority(params.walletPda, credentialOrPubkey);
    const s = params.ownerSigner;

    if (s.type === 'ed25519') {
      const ix = createTransferOwnershipIx({
        payer: params.payer, walletPda: params.walletPda,
        currentOwnerAuthorityPda: s.authorityPda ?? this.findAuthority(params.walletPda, s.publicKey.toBytes())[0],
        newOwnerAuthorityPda, newType, credentialOrPubkey, secp256r1Pubkey, rpId,
        authorizerSigner: s.publicKey, programId: this.programId,
      });
      return { instructions: [ix], newOwnerAuthorityPda };
    }

    const currentOwnerAuthorityPda = s.authorityPda
      ?? this.findAuthority(params.walletPda, s.signer.credentialIdHash)[0];
    const slot = s.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(currentOwnerAuthorityPda)) + 1;

    const dataPayload = buildDataPayloadForTransfer(newType, credentialOrPubkey, secp256r1Pubkey, rpId);
    const signedPayload = concatParts([dataPayload, params.payer.toBytes()]);

    const { authPayload, precompileIx } = await signWithSecp256r1({
      signer: s.signer, discriminator: new Uint8Array([DISC_TRANSFER_OWNERSHIP]),
      signedPayload, sysvarIxIndex: SYSVAR_IX_INDEX_TRANSFER_OWNERSHIP,
      slot, counter, payer: params.payer, programId: this.programId,
    });

    const ix = createTransferOwnershipIx({
      payer: params.payer, walletPda: params.walletPda,
      currentOwnerAuthorityPda, newOwnerAuthorityPda, newType,
      credentialOrPubkey, secp256r1Pubkey, rpId,
      authPayload, programId: this.programId,
    });
    return { instructions: [precompileIx, ix], newOwnerAuthorityPda };
  }

  // ─── CreateSession (unified) ────────────────────────────────────

  /**
   * Create a session key for the wallet.
   *
   * @example
   * ```typescript
   * const { instructions, sessionPda } = await client.createSession({
   *   payer: payer.publicKey,
   *   walletPda,
   *   adminSigner: ed25519(ownerKp.publicKey),
   *   sessionKey: sessionKp.publicKey,
   *   expiresAt: currentSlot + 9000n,
   * });
   * ```
   */
  async createSession(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    adminSigner: AdminSigner;
    sessionKey: PublicKey;
    expiresAt: bigint;
  }): Promise<{ instructions: TransactionInstruction[]; sessionPda: PublicKey }> {
    const sessionKeyBytes = params.sessionKey.toBytes();
    const [sessionPda] = this.findSession(params.walletPda, sessionKeyBytes);
    const s = params.adminSigner;

    if (s.type === 'ed25519') {
      const ix = createCreateSessionIx({
        payer: params.payer, walletPda: params.walletPda,
        adminAuthorityPda: s.authorityPda ?? this.findAuthority(params.walletPda, s.publicKey.toBytes())[0],
        sessionPda, sessionKey: sessionKeyBytes, expiresAt: params.expiresAt,
        authorizerSigner: s.publicKey, programId: this.programId,
      });
      return { instructions: [ix], sessionPda };
    }

    const adminAuthorityPda = s.authorityPda
      ?? this.findAuthority(params.walletPda, s.signer.credentialIdHash)[0];
    const slot = s.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(adminAuthorityPda)) + 1;

    const dataPayload = buildDataPayloadForSession(sessionKeyBytes, params.expiresAt);
    const signedPayload = concatParts([dataPayload, params.payer.toBytes()]);

    const { authPayload, precompileIx } = await signWithSecp256r1({
      signer: s.signer, discriminator: new Uint8Array([DISC_CREATE_SESSION]),
      signedPayload, sysvarIxIndex: SYSVAR_IX_INDEX_CREATE_SESSION,
      slot, counter, payer: params.payer, programId: this.programId,
    });

    const ix = createCreateSessionIx({
      payer: params.payer, walletPda: params.walletPda, adminAuthorityPda, sessionPda,
      sessionKey: sessionKeyBytes, expiresAt: params.expiresAt,
      authPayload, programId: this.programId,
    });
    return { instructions: [precompileIx, ix], sessionPda };
  }

  // ─── Execute (unified, accepts standard TransactionInstructions) ─

  /**
   * Execute arbitrary Solana instructions via the wallet.
   *
   * Works with any signer type: Ed25519, Secp256r1 (passkey), or Session key.
   * Pass standard `TransactionInstruction[]` — the SDK handles compact encoding,
   * account indexing, and signing automatically.
   *
   * @example
   * ```typescript
   * const [vault] = client.findVault(walletPda);
   * const { instructions } = await client.execute({
   *   payer: payer.publicKey,
   *   walletPda,
   *   signer: secp256r1(mySigner),
   *   instructions: [
   *     SystemProgram.transfer({ fromPubkey: vault, toPubkey: recipient, lamports: 1_000_000 }),
   *   ],
   * });
   * await sendAndConfirmTransaction(connection, new Transaction().add(...instructions), [payer]);
   * ```
   */
  async execute(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    signer: ExecuteSigner;
    instructions: TransactionInstruction[];
  }): Promise<{ instructions: TransactionInstruction[] }> {
    const [vaultPda] = this.findVault(params.walletPda);
    const s = params.signer;

    switch (s.type) {
      case 'ed25519': {
        const authorityPda = s.authorityPda
          ?? this.findAuthority(params.walletPda, s.publicKey.toBytes())[0];
        // Ed25519: signer at index 4 (program expects it there)
        const fixedAccounts = [params.payer, params.walletPda, authorityPda, vaultPda, s.publicKey];
        const { compactInstructions, remainingAccounts } = buildCompactLayout(fixedAccounts, params.instructions);
        const packed = packCompactInstructions(compactInstructions);
        const ix = createExecuteIx({
          payer: params.payer, walletPda: params.walletPda,
          authorityPda, vaultPda, packedInstructions: packed,
          authorizerSigner: s.publicKey,
          remainingAccounts, programId: this.programId,
        });
        return { instructions: [ix] };
      }

      case 'secp256r1': {
        const authorityPda = s.authorityPda
          ?? this.findAuthority(params.walletPda, s.signer.credentialIdHash)[0];
        const slot = s.slotOverride ?? BigInt(await this.connection.getSlot());
        const counter = (await this.readCounter(authorityPda)) + 1;

        // Secp256r1: sysvar_instructions at index 4
        const fixedAccounts = [params.payer, params.walletPda, authorityPda, vaultPda, SYSVAR_INSTRUCTIONS_PUBKEY];
        const { compactInstructions, remainingAccounts } = buildCompactLayout(fixedAccounts, params.instructions);
        const packed = packCompactInstructions(compactInstructions);

        // Compute accounts hash for signature binding
        const allAccountMetas = [
          { pubkey: params.payer, isSigner: true, isWritable: false },
          { pubkey: params.walletPda, isSigner: false, isWritable: false },
          { pubkey: authorityPda, isSigner: false, isWritable: true },
          { pubkey: vaultPda, isSigner: false, isWritable: true },
          { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
          ...remainingAccounts,
        ];
        const accountsHash = computeAccountsHash(allAccountMetas, compactInstructions);
        const signedPayload = concatParts([packed, accountsHash]);

        const { authPayload, precompileIx } = await signWithSecp256r1({
          signer: s.signer, discriminator: new Uint8Array([DISC_EXECUTE]),
          signedPayload, sysvarIxIndex: SYSVAR_IX_INDEX_EXECUTE,
          slot, counter, payer: params.payer, programId: this.programId,
        });

        const ix = createExecuteIx({
          payer: params.payer, walletPda: params.walletPda,
          authorityPda, vaultPda, packedInstructions: packed,
          authPayload, remainingAccounts, programId: this.programId,
        });
        return { instructions: [precompileIx, ix] };
      }

      case 'session': {
        // Session: sessionKey as signer is included in fixed accounts for index mapping
        const fixedAccounts = [params.payer, params.walletPda, s.sessionPda, vaultPda, s.sessionKeyPubkey];
        const { compactInstructions, remainingAccounts } = buildCompactLayout(fixedAccounts, params.instructions);
        const packed = packCompactInstructions(compactInstructions);

        // Session key must be prepended to remaining accounts as a signer
        const sessionKeyMeta = { pubkey: s.sessionKeyPubkey, isSigner: true, isWritable: false };
        const allRemaining = [sessionKeyMeta, ...remainingAccounts];

        const ix = createExecuteIx({
          payer: params.payer, walletPda: params.walletPda,
          authorityPda: s.sessionPda, vaultPda, packedInstructions: packed,
          remainingAccounts: allRemaining, programId: this.programId,
        });
        return { instructions: [ix] };
      }
    }
  }

  // ─── TransferSol (convenience) ──────────────────────────────────

  /**
   * Transfer SOL from the wallet vault to a recipient.
   * Works with any signer type.
   *
   * @example
   * ```typescript
   * const { instructions } = await client.transferSol({
   *   payer: payer.publicKey,
   *   walletPda,
   *   signer: secp256r1(mySigner),
   *   recipient: destination,
   *   lamports: 1_000_000n,
   * });
   * ```
   */
  async transferSol(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    signer: ExecuteSigner;
    recipient: PublicKey;
    lamports: bigint | number;
  }): Promise<{ instructions: TransactionInstruction[] }> {
    const [vaultPda] = this.findVault(params.walletPda);
    const amount = typeof params.lamports === 'bigint' ? Number(params.lamports) : params.lamports;

    return this.execute({
      payer: params.payer, walletPda: params.walletPda, signer: params.signer,
      instructions: [SystemProgram.transfer({ fromPubkey: vaultPda, toPubkey: params.recipient, lamports: amount })],
    });
  }

  // ─── Authorize (deferred execution TX1) ─────────────────────────

  /**
   * Authorize deferred execution. Pass standard TransactionInstructions
   * — the SDK handles compact encoding and hash computation.
   *
   * Returns pre-computed `deferredPayload` for TX2.
   */
  async authorize(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    signer: Secp256r1SignerConfig;
    /** Standard instructions to defer */
    instructions: TransactionInstruction[];
    /** Expiry offset in slots (default 300 = ~2 minutes) */
    expiryOffset?: number;
  }): Promise<{
    instructions: TransactionInstruction[];
    deferredExecPda: PublicKey;
    counter: number;
    deferredPayload: DeferredPayload;
  }> {
    const [vaultPda] = this.findVault(params.walletPda);
    const s = params.signer;
    const authorityPda = s.authorityPda
      ?? this.findAuthority(params.walletPda, s.signer.credentialIdHash)[0];
    const slot = s.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(authorityPda)) + 1;
    const expiryOffset = params.expiryOffset ?? 300;

    const [deferredExecPda] = this.findDeferredExec(params.walletPda, authorityPda, counter);

    // TX2 fixed accounts: payer, wallet, vault, deferred, refund_destination (=payer)
    const tx2FixedAccounts = [params.payer, params.walletPda, vaultPda, deferredExecPda, params.payer];
    const { compactInstructions, remainingAccounts } = buildCompactLayout(tx2FixedAccounts, params.instructions);

    // Compute hashes
    const instructionsHash = computeInstructionsHash(compactInstructions);
    const tx2AccountMetas = [
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: params.walletPda, isSigner: false, isWritable: true },
      { pubkey: vaultPda, isSigner: false, isWritable: true },
      { pubkey: deferredExecPda, isSigner: false, isWritable: true },
      { pubkey: params.payer, isSigner: false, isWritable: true }, // refund dest
      ...remainingAccounts,
    ];
    const accountsHash = computeAccountsHash(tx2AccountMetas, compactInstructions);
    const expiryOffsetBuf = new Uint8Array(2);
    expiryOffsetBuf[0] = expiryOffset & 0xff;
    expiryOffsetBuf[1] = (expiryOffset >> 8) & 0xff;
    const signedPayload = concatParts([instructionsHash, accountsHash, expiryOffsetBuf]);

    const { authPayload, precompileIx } = await signWithSecp256r1({
      signer: s.signer, discriminator: new Uint8Array([DISC_AUTHORIZE]),
      signedPayload, sysvarIxIndex: SYSVAR_IX_INDEX_AUTHORIZE,
      slot, counter, payer: params.payer, programId: this.programId,
    });

    const authorizeIx = createAuthorizeIx({
      payer: params.payer, walletPda: params.walletPda, authorityPda, deferredExecPda,
      instructionsHash, accountsHash, expiryOffset, authPayload,
      programId: this.programId,
    });

    return {
      instructions: [precompileIx, authorizeIx],
      deferredExecPda,
      counter,
      deferredPayload: { walletPda: params.walletPda, deferredExecPda, compactInstructions, remainingAccounts },
    };
  }

  // ─── ExecuteDeferred (from payload) ─────────────────────────────

  /**
   * Build TX2 from the payload returned by `authorize()`.
   */
  executeDeferredFromPayload(params: {
    payer: PublicKey;
    deferredPayload: DeferredPayload;
    refundDestination?: PublicKey;
  }): { instructions: TransactionInstruction[] } {
    const [vaultPda] = this.findVault(params.deferredPayload.walletPda);
    const refundDest = params.refundDestination ?? params.payer;
    const packed = packCompactInstructions(params.deferredPayload.compactInstructions);
    const ix = createExecuteDeferredIx({
      payer: params.payer, walletPda: params.deferredPayload.walletPda, vaultPda,
      deferredExecPda: params.deferredPayload.deferredExecPda,
      refundDestination: refundDest, packedInstructions: packed,
      remainingAccounts: params.deferredPayload.remainingAccounts,
      programId: this.programId,
    });
    return { instructions: [ix] };
  }

  // ─── ReclaimDeferred ────────────────────────────────────────────

  reclaimDeferred(params: {
    payer: PublicKey;
    deferredExecPda: PublicKey;
    refundDestination?: PublicKey;
  }): { instructions: TransactionInstruction[] } {
    const ix = createReclaimDeferredIx({
      payer: params.payer,
      deferredExecPda: params.deferredExecPda,
      refundDestination: params.refundDestination ?? params.payer,
      programId: this.programId,
    });
    return { instructions: [ix] };
  }

  // ─── RevokeSession ─────────────────────────────────────────────

  /**
   * Revoke a session key early (before expiry).
   * Only Owner or Admin can revoke. Refunds session rent.
   *
   * @example Revoke with Ed25519 admin
   * ```typescript
   * const { instructions } = await client.revokeSession({
   *   payer: payer.publicKey,
   *   walletPda,
   *   adminSigner: ed25519(adminKp.publicKey, adminAuthorityPda),
   *   sessionPda,
   * });
   * ```
   */
  async revokeSession(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    adminSigner: AdminSigner;
    sessionPda: PublicKey;
    refundDestination?: PublicKey;
  }): Promise<{ instructions: TransactionInstruction[] }> {
    const refundDest = params.refundDestination ?? params.payer;
    const s = params.adminSigner;

    if (s.type === 'ed25519') {
      const ix = createRevokeSessionIx({
        payer: params.payer, walletPda: params.walletPda,
        adminAuthorityPda: s.authorityPda ?? this.findAuthority(params.walletPda, s.publicKey.toBytes())[0],
        sessionPda: params.sessionPda, refundDestination: refundDest,
        authorizerSigner: s.publicKey, programId: this.programId,
      });
      return { instructions: [ix] };
    }

    const adminAuthorityPda = s.authorityPda
      ?? this.findAuthority(params.walletPda, s.signer.credentialIdHash)[0];
    const slot = s.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(adminAuthorityPda)) + 1;

    const signedPayload = concatParts([
      params.sessionPda.toBytes(), refundDest.toBytes(),
    ]);

    const { authPayload, precompileIx } = await signWithSecp256r1({
      signer: s.signer, discriminator: new Uint8Array([DISC_REVOKE_SESSION]),
      signedPayload, sysvarIxIndex: SYSVAR_IX_INDEX_REVOKE_SESSION,
      slot, counter, payer: params.payer, programId: this.programId,
    });

    const ix = createRevokeSessionIx({
      payer: params.payer, walletPda: params.walletPda, adminAuthorityPda,
      sessionPda: params.sessionPda, refundDestination: refundDest,
      authPayload, programId: this.programId,
    });
    return { instructions: [precompileIx, ix] };
  }
}
