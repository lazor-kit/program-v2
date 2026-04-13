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
import {
  readAuthorityCounter,
  buildAuthPayload,
  buildSecp256r1Challenge,
  generateAuthenticatorData,
  type Secp256r1Signer,
} from './secp256r1';
import type { Ed25519Signer } from './ed25519';
import { packCompactInstructions, computeAccountsHash, computeInstructionsHash, type CompactInstruction } from './packing';
import {
  createCreateWalletIx,
  createAddAuthorityIx,
  createRemoveAuthorityIx,
  createTransferOwnershipIx,
  createExecuteIx,
  createCreateSessionIx,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  DISC_ADD_AUTHORITY,
  DISC_REMOVE_AUTHORITY,
  DISC_TRANSFER_OWNERSHIP,
  DISC_EXECUTE,
  DISC_CREATE_SESSION,
  DISC_AUTHORIZE,
  createAuthorizeIx,
  createExecuteDeferredIx,
  createReclaimDeferredIx,
} from './instructions';

// ─── Sysvar instruction indexes (auto-computed from account layouts) ──

/** AddAuthority: payer=0, wallet=1, admin=2, newAuth=3, system=4, rent=5, sysvar=6 */
const SYSVAR_IX_INDEX_ADD_AUTHORITY = 6;
/** RemoveAuthority: payer=0, wallet=1, admin=2, target=3, refund=4, sysvar=5 */
const SYSVAR_IX_INDEX_REMOVE_AUTHORITY = 5;
/** TransferOwnership: payer=0, wallet=1, current=2, new=3, system=4, rent=5, sysvar=6 */
const SYSVAR_IX_INDEX_TRANSFER_OWNERSHIP = 6;
/** Execute: payer=0, wallet=1, auth=2, vault=3, sysvar=4 (before remaining accounts) */
const SYSVAR_IX_INDEX_EXECUTE = 4;
/** CreateSession: payer=0, wallet=1, admin=2, session=3, system=4, rent=5, sysvar=6 */
const SYSVAR_IX_INDEX_CREATE_SESSION = 6;
/** Authorize: payer=0, wallet=1, auth=2, deferred=3, system=4, rent=5, sysvar=6 */
const SYSVAR_IX_INDEX_AUTHORIZE = 6;

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

  // ─── Internal: Secp256r1 signing helper ─────────────────────────
  /**
   * Centralizes the Secp256r1 signing flow:
   * 1. Generate authenticatorData from signer's rpId
   * 2. Build authPayload (with authenticatorData, slot, counter, sysvarIxIndex)
   * 3. Compute challenge hash (includes authPayload + signedPayload)
   * 4. Call signer.sign(challenge) to get signature + precompile data
   * 5. Build precompile instruction
   */
  private async signWithSecp256r1(params: {
    signer: Secp256r1Signer;
    discriminator: Uint8Array;
    signedPayload: Uint8Array;
    sysvarIxIndex: number;
    slot: bigint;
    counter: number;
    payer: PublicKey;
  }): Promise<{
    authPayload: Uint8Array;
    precompileIx: TransactionInstruction;
  }> {
    // 1. Generate authenticatorData deterministically from rpId
    const authenticatorData = generateAuthenticatorData(params.signer.rpId);

    // 2. Build authPayload (included in challenge hash)
    const authPayload = buildAuthPayload({
      slot: params.slot,
      counter: params.counter,
      sysvarIxIndex: params.sysvarIxIndex,
      typeAndFlags: 0x10, // webauthn.get + https
      authenticatorData,
    });

    // 3. Compute challenge hash
    const challenge = buildSecp256r1Challenge({
      discriminator: params.discriminator,
      authPayload,
      signedPayload: params.signedPayload,
      slot: params.slot,
      payer: params.payer,
      counter: params.counter,
      programId: this.programId,
    });

    // 4. Sign — signer returns signature + authenticatorData + clientDataJsonHash
    const { signature, authenticatorData: signerAuthData, clientDataJsonHash } =
      await params.signer.sign(challenge);

    // 5. Build precompile: message = authenticatorData + clientDataJsonHash
    const precompileMessage = concatParts([signerAuthData, clientDataJsonHash]);
    const precompileIx = buildSecp256r1PrecompileIx(
      params.signer.publicKeyBytes,
      precompileMessage,
      signature,
    );

    return { authPayload, precompileIx };
  }

  // ─── CreateWallet (Ed25519) ──────────────────────────────────────
  createWalletEd25519(params: {
    payer: PublicKey;
    userSeed: Uint8Array;
    ownerPubkey: PublicKey;
  }): { ix: TransactionInstruction; walletPda: PublicKey; vaultPda: PublicKey; authorityPda: PublicKey } {
    const pubkeyBytes = params.ownerPubkey.toBytes();
    const credIdHash = pubkeyBytes; // Ed25519 uses pubkey as credential hash
    const [walletPda] = this.findWallet(params.userSeed);
    const [vaultPda] = this.findVault(walletPda);
    const [authorityPda, authBump] = this.findAuthority(walletPda, credIdHash);

    const ix = createCreateWalletIx({
      payer: params.payer,
      walletPda,
      vaultPda,
      authorityPda,
      userSeed: params.userSeed,
      authType: AUTH_TYPE_ED25519,
      authBump,
      credentialOrPubkey: pubkeyBytes,
      programId: this.programId,
    });

    return { ix, walletPda, vaultPda, authorityPda };
  }

  // ─── CreateWallet (Secp256r1) ────────────────────────────────────
  createWalletSecp256r1(params: {
    payer: PublicKey;
    userSeed: Uint8Array;
    credentialIdHash: Uint8Array;
    compressedPubkey: Uint8Array;
    rpId: string;
  }): { ix: TransactionInstruction; walletPda: PublicKey; vaultPda: PublicKey; authorityPda: PublicKey } {
    const [walletPda] = this.findWallet(params.userSeed);
    const [vaultPda] = this.findVault(walletPda);
    const [authorityPda, authBump] = this.findAuthority(walletPda, params.credentialIdHash);

    const ix = createCreateWalletIx({
      payer: params.payer,
      walletPda,
      vaultPda,
      authorityPda,
      userSeed: params.userSeed,
      authType: AUTH_TYPE_SECP256R1,
      authBump,
      credentialOrPubkey: params.credentialIdHash,
      secp256r1Pubkey: params.compressedPubkey,
      rpId: params.rpId,
      programId: this.programId,
    });

    return { ix, walletPda, vaultPda, authorityPda };
  }

  // ─── AddAuthority (Ed25519 admin) ────────────────────────────────
  addAuthorityEd25519(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    adminAuthorityPda: PublicKey;
    adminSigner: PublicKey;
    newType: number;
    newRole: number;
    newCredentialOrPubkey: Uint8Array;
    newSecp256r1Pubkey?: Uint8Array;
    newRpId?: string;
  }): { ix: TransactionInstruction; newAuthorityPda: PublicKey } {
    const credHash = params.newCredentialOrPubkey;
    const [newAuthorityPda] = this.findAuthority(params.walletPda, credHash);

    const ix = createAddAuthorityIx({
      payer: params.payer,
      walletPda: params.walletPda,
      adminAuthorityPda: params.adminAuthorityPda,
      newAuthorityPda,
      newType: params.newType,
      newRole: params.newRole,
      credentialOrPubkey: params.newCredentialOrPubkey,
      secp256r1Pubkey: params.newSecp256r1Pubkey,
      rpId: params.newRpId,
      authorizerSigner: params.adminSigner,
      programId: this.programId,
    });

    return { ix, newAuthorityPda };
  }

  // ─── AddAuthority (Secp256r1 admin) ──────────────────────────────
  async addAuthoritySecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    /** Auto-derived from adminSigner.credentialIdHash if omitted */
    adminAuthorityPda?: PublicKey;
    adminSigner: Secp256r1Signer;
    newType: number;
    newRole: number;
    newCredentialOrPubkey: Uint8Array;
    newSecp256r1Pubkey?: Uint8Array;
    newRpId?: string;
    /** Override slot (auto-fetched from connection if omitted) */
    slotOverride?: bigint;
  }): Promise<{ ix: TransactionInstruction; newAuthorityPda: PublicKey; precompileIx: TransactionInstruction }> {
    const credHash = params.newCredentialOrPubkey;
    const [newAuthorityPda] = this.findAuthority(params.walletPda, credHash);
    const adminAuthorityPda = params.adminAuthorityPda
      ?? this.findAuthority(params.walletPda, params.adminSigner.credentialIdHash)[0];

    const slot = params.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(adminAuthorityPda)) + 1;

    // Build signed payload: dataPayload + payer (on-chain extends with payer)
    const dataPayload = buildDataPayloadForAdd(
      params.newType, params.newRole,
      params.newCredentialOrPubkey, params.newSecp256r1Pubkey,
      params.newRpId,
    );
    const signedPayload = concatParts([dataPayload, params.payer.toBytes()]);

    const { authPayload, precompileIx } = await this.signWithSecp256r1({
      signer: params.adminSigner,
      discriminator: new Uint8Array([DISC_ADD_AUTHORITY]),
      signedPayload,
      sysvarIxIndex: SYSVAR_IX_INDEX_ADD_AUTHORITY,
      slot,
      counter,
      payer: params.payer,
    });

    const ix = createAddAuthorityIx({
      payer: params.payer,
      walletPda: params.walletPda,
      adminAuthorityPda,
      newAuthorityPda,
      newType: params.newType,
      newRole: params.newRole,
      credentialOrPubkey: params.newCredentialOrPubkey,
      secp256r1Pubkey: params.newSecp256r1Pubkey,
      rpId: params.newRpId,
      authPayload,
      programId: this.programId,
    });

    return { ix, newAuthorityPda, precompileIx };
  }

  // ─── RemoveAuthority (Ed25519 admin) ─────────────────────────────
  removeAuthorityEd25519(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    adminAuthorityPda: PublicKey;
    adminSigner: PublicKey;
    targetAuthorityPda: PublicKey;
    refundDestination?: PublicKey;
  }): TransactionInstruction {
    const refundDest = params.refundDestination ?? params.payer;
    return createRemoveAuthorityIx({
      payer: params.payer,
      walletPda: params.walletPda,
      adminAuthorityPda: params.adminAuthorityPda,
      targetAuthorityPda: params.targetAuthorityPda,
      refundDestination: refundDest,
      authorizerSigner: params.adminSigner,
      programId: this.programId,
    });
  }

  // ─── RemoveAuthority (Secp256r1 admin) ───────────────────────────
  async removeAuthoritySecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    /** Auto-derived from adminSigner.credentialIdHash if omitted */
    adminAuthorityPda?: PublicKey;
    adminSigner: Secp256r1Signer;
    targetAuthorityPda: PublicKey;
    refundDestination?: PublicKey;
    /** Override slot (auto-fetched from connection if omitted) */
    slotOverride?: bigint;
  }): Promise<{ ix: TransactionInstruction; precompileIx: TransactionInstruction }> {
    const refundDest = params.refundDestination ?? params.payer;
    const adminAuthorityPda = params.adminAuthorityPda
      ?? this.findAuthority(params.walletPda, params.adminSigner.credentialIdHash)[0];
    const slot = params.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(adminAuthorityPda)) + 1;

    // On-chain: signed_payload = target_key || refund_key (NO payer extension)
    const signedPayload = concatParts([
      params.targetAuthorityPda.toBytes(),
      refundDest.toBytes(),
    ]);

    const { authPayload, precompileIx } = await this.signWithSecp256r1({
      signer: params.adminSigner,
      discriminator: new Uint8Array([DISC_REMOVE_AUTHORITY]),
      signedPayload,
      sysvarIxIndex: SYSVAR_IX_INDEX_REMOVE_AUTHORITY,
      slot,
      counter,
      payer: params.payer,
    });

    const ix = createRemoveAuthorityIx({
      payer: params.payer,
      walletPda: params.walletPda,
      adminAuthorityPda,
      targetAuthorityPda: params.targetAuthorityPda,
      refundDestination: refundDest,
      authPayload,
      programId: this.programId,
    });

    return { ix, precompileIx };
  }

  // ─── Execute (Ed25519) ───────────────────────────────────────────
  executeEd25519(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    authorityPda: PublicKey;
    compactInstructions: CompactInstruction[];
    remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  }): TransactionInstruction {
    const [vaultPda] = this.findVault(params.walletPda);
    const packed = packCompactInstructions(params.compactInstructions);
    return createExecuteIx({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityPda: params.authorityPda,
      vaultPda,
      packedInstructions: packed,
      remainingAccounts: params.remainingAccounts,
      programId: this.programId,
    });
  }

  // ─── Execute (Secp256r1) ─────────────────────────────────────────
  async executeSecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    /** Auto-derived from signer.credentialIdHash if omitted */
    authorityPda?: PublicKey;
    signer: Secp256r1Signer;
    compactInstructions: CompactInstruction[];
    remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
    /** Override slot (auto-fetched from connection if omitted) */
    slotOverride?: bigint;
  }): Promise<{ ix: TransactionInstruction; precompileIx: TransactionInstruction }> {
    const authorityPda = params.authorityPda
      ?? this.findAuthority(params.walletPda, params.signer.credentialIdHash)[0];
    const [vaultPda] = this.findVault(params.walletPda);
    const slot = params.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(authorityPda)) + 1;
    const packed = packCompactInstructions(params.compactInstructions);

    // Build full account list for accountsHash computation
    // Execute layout: payer=0, wallet=1, authority=2, vault=3, sysvar_ix=4, remaining=5+
    const allAccountMetas = [
      { pubkey: params.payer, isSigner: true, isWritable: false },
      { pubkey: params.walletPda, isSigner: false, isWritable: false },
      { pubkey: authorityPda, isSigner: false, isWritable: true },
      { pubkey: vaultPda, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false }, // sysvar placeholder
      ...(params.remainingAccounts ?? []),
    ];
    const accountsHash = computeAccountsHash(allAccountMetas, params.compactInstructions);

    // On-chain: signed_payload = compact_bytes + accounts_hash
    const signedPayload = concatParts([packed, accountsHash]);

    const { authPayload, precompileIx } = await this.signWithSecp256r1({
      signer: params.signer,
      discriminator: new Uint8Array([DISC_EXECUTE]),
      signedPayload,
      sysvarIxIndex: SYSVAR_IX_INDEX_EXECUTE,
      slot,
      counter,
      payer: params.payer,
    });

    const ix = createExecuteIx({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityPda,
      vaultPda,
      packedInstructions: packed,
      authPayload,
      remainingAccounts: params.remainingAccounts,
      programId: this.programId,
    });

    return { ix, precompileIx };
  }

  // ─── Execute (Session key) ──────────────────────────────────────
  /**
   * Execute via session key. The session key must be added as a tx signer
   * when building the Transaction (e.g., `[payer, sessionKeypair]`).
   *
   * Note: The session key pubkey is prepended to remainingAccounts as a signer.
   * Compact instruction account indexes should account for this layout:
   *   0: payer, 1: wallet, 2: sessionPda, 3: vault,
   *   4: sessionKey (signer), 5+: your remaining accounts
   */
  executeSession(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    sessionPda: PublicKey;
    sessionKeyPubkey: PublicKey;
    compactInstructions: CompactInstruction[];
    remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  }): TransactionInstruction {
    const [vaultPda] = this.findVault(params.walletPda);
    const packed = packCompactInstructions(params.compactInstructions);

    // Session key must be a signer — prepend it to remaining accounts
    const sessionKeyMeta = { pubkey: params.sessionKeyPubkey, isSigner: true, isWritable: false };
    const allRemainingAccounts = [sessionKeyMeta, ...(params.remainingAccounts ?? [])];

    return createExecuteIx({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityPda: params.sessionPda,
      vaultPda,
      packedInstructions: packed,
      // No authPayload — session uses Ed25519 signer-based auth
      remainingAccounts: allRemainingAccounts,
      programId: this.programId,
    });
  }

  // ─── High-level: execute standard instructions (Secp256r1) ──────
  /**
   * Execute arbitrary Solana instructions via the wallet with Secp256r1 auth.
   *
   * Pass **standard TransactionInstructions** (e.g., `SystemProgram.transfer(...)`)
   * — the SDK automatically handles compact instruction encoding, account indexing,
   * and Secp256r1 signing.
   *
   * Use `client.findVault(walletPda)[0]` as the source account in your instructions.
   *
   * @returns Array of instructions ready to add to a Transaction: [precompileIx, executeIx]
   *
   * @example
   * ```typescript
   * const [vaultPda] = client.findVault(walletPda);
   * const ixs = await client.execute({
   *   payer: payer.publicKey,
   *   walletPda,
   *   signer,
   *   instructions: [
   *     SystemProgram.transfer({ fromPubkey: vaultPda, toPubkey: recipient, lamports: 1_000_000 }),
   *   ],
   * });
   * await sendAndConfirmTransaction(connection, new Transaction().add(...ixs), [payer]);
   * ```
   */
  async execute(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    signer: Secp256r1Signer;
    /** Standard Solana TransactionInstructions to execute via the wallet vault. */
    instructions: TransactionInstruction[];
    /** Override slot (auto-fetched from connection if omitted) */
    slotOverride?: bigint;
  }): Promise<TransactionInstruction[]> {
    const [authorityPda] = this.findAuthority(params.walletPda, params.signer.credentialIdHash);
    const [vaultPda] = this.findVault(params.walletPda);

    // Fixed accounts in the Execute instruction layout:
    //   0: payer, 1: wallet, 2: authority, 3: vault, 4: sysvar_instructions
    const fixedAccounts = [
      params.payer,
      params.walletPda,
      authorityPda,
      vaultPda,
      SYSVAR_INSTRUCTIONS_PUBKEY, // sysvar placeholder (not referenced by compact instructions)
    ];

    const { compactInstructions, remainingAccounts } = buildCompactLayout(
      fixedAccounts,
      params.instructions,
    );

    const { ix, precompileIx } = await this.executeSecp256r1({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityPda,
      signer: params.signer,
      compactInstructions,
      remainingAccounts,
      slotOverride: params.slotOverride,
    });

    return [precompileIx, ix];
  }

  // ─── High-level: SOL transfer (Secp256r1) ──────────────────────
  /**
   * Transfer SOL from the wallet vault to a recipient.
   *
   * The simplest way to send SOL — no compact instructions, no account
   * indexing, no signing details.
   *
   * @returns Array of instructions ready to add to a Transaction: [precompileIx, executeIx]
   *
   * @example
   * ```typescript
   * const ixs = await client.transferSol({
   *   payer: payer.publicKey,
   *   walletPda,
   *   signer,
   *   recipient: recipientPubkey,
   *   lamports: 1_000_000,
   * });
   * await sendAndConfirmTransaction(connection, new Transaction().add(...ixs), [payer]);
   * ```
   */
  async transferSol(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    signer: Secp256r1Signer;
    recipient: PublicKey;
    lamports: bigint | number;
    /** Override slot (auto-fetched from connection if omitted) */
    slotOverride?: bigint;
  }): Promise<TransactionInstruction[]> {
    const [vaultPda] = this.findVault(params.walletPda);
    const amount = typeof params.lamports === 'bigint'
      ? Number(params.lamports)
      : params.lamports;

    const transferIx = SystemProgram.transfer({
      fromPubkey: vaultPda,
      toPubkey: params.recipient,
      lamports: amount,
    });

    return this.execute({
      payer: params.payer,
      walletPda: params.walletPda,
      signer: params.signer,
      instructions: [transferIx],
      slotOverride: params.slotOverride,
    });
  }

  // ─── CreateSession (Ed25519) ─────────────────────────────────────
  createSessionEd25519(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    adminAuthorityPda: PublicKey;
    adminSigner: PublicKey;
    sessionKey: Uint8Array;
    expiresAt: bigint;
  }): { ix: TransactionInstruction; sessionPda: PublicKey } {
    const [sessionPda] = this.findSession(params.walletPda, params.sessionKey);

    const ix = createCreateSessionIx({
      payer: params.payer,
      walletPda: params.walletPda,
      adminAuthorityPda: params.adminAuthorityPda,
      sessionPda,
      sessionKey: params.sessionKey,
      expiresAt: params.expiresAt,
      authorizerSigner: params.adminSigner,
      programId: this.programId,
    });

    return { ix, sessionPda };
  }

  // ─── CreateSession (Secp256r1) ───────────────────────────────────
  async createSessionSecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    /** Auto-derived from adminSigner.credentialIdHash if omitted */
    adminAuthorityPda?: PublicKey;
    adminSigner: Secp256r1Signer;
    sessionKey: Uint8Array;
    expiresAt: bigint;
    /** Override slot (auto-fetched from connection if omitted) */
    slotOverride?: bigint;
  }): Promise<{ ix: TransactionInstruction; sessionPda: PublicKey; precompileIx: TransactionInstruction }> {
    const [sessionPda] = this.findSession(params.walletPda, params.sessionKey);
    const adminAuthorityPda = params.adminAuthorityPda
      ?? this.findAuthority(params.walletPda, params.adminSigner.credentialIdHash)[0];
    const slot = params.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(adminAuthorityPda)) + 1;

    // On-chain: data_payload = [session_key(32)][expires_at(8)]
    // signed_payload = data_payload + payer.key()
    const dataPayload = buildDataPayloadForSession(params.sessionKey, params.expiresAt);
    const signedPayload = concatParts([dataPayload, params.payer.toBytes()]);

    const { authPayload, precompileIx } = await this.signWithSecp256r1({
      signer: params.adminSigner,
      discriminator: new Uint8Array([DISC_CREATE_SESSION]),
      signedPayload,
      sysvarIxIndex: SYSVAR_IX_INDEX_CREATE_SESSION,
      slot,
      counter,
      payer: params.payer,
    });

    const ix = createCreateSessionIx({
      payer: params.payer,
      walletPda: params.walletPda,
      adminAuthorityPda,
      sessionPda,
      sessionKey: params.sessionKey,
      expiresAt: params.expiresAt,
      authPayload,
      programId: this.programId,
    });

    return { ix, sessionPda, precompileIx };
  }

  // ─── TransferOwnership (Ed25519) ─────────────────────────────────
  transferOwnershipEd25519(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    currentOwnerAuthorityPda: PublicKey;
    ownerSigner: PublicKey;
    newType: number;
    newCredentialOrPubkey: Uint8Array;
    newSecp256r1Pubkey?: Uint8Array;
    newRpId?: string;
  }): { ix: TransactionInstruction; newOwnerAuthorityPda: PublicKey } {
    const credHash = params.newCredentialOrPubkey;
    const [newOwnerAuthorityPda] = this.findAuthority(params.walletPda, credHash);

    const ix = createTransferOwnershipIx({
      payer: params.payer,
      walletPda: params.walletPda,
      currentOwnerAuthorityPda: params.currentOwnerAuthorityPda,
      newOwnerAuthorityPda,
      newType: params.newType,
      credentialOrPubkey: params.newCredentialOrPubkey,
      secp256r1Pubkey: params.newSecp256r1Pubkey,
      rpId: params.newRpId,
      authorizerSigner: params.ownerSigner,
      programId: this.programId,
    });

    return { ix, newOwnerAuthorityPda };
  }

  // ─── TransferOwnership (Secp256r1) ───────────────────────────────
  async transferOwnershipSecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    /** Auto-derived from ownerSigner.credentialIdHash if omitted */
    currentOwnerAuthorityPda?: PublicKey;
    ownerSigner: Secp256r1Signer;
    newType: number;
    newCredentialOrPubkey: Uint8Array;
    newSecp256r1Pubkey?: Uint8Array;
    newRpId?: string;
    /** Override slot (auto-fetched from connection if omitted) */
    slotOverride?: bigint;
  }): Promise<{ ix: TransactionInstruction; newOwnerAuthorityPda: PublicKey; precompileIx: TransactionInstruction }> {
    const credHash = params.newCredentialOrPubkey;
    const [newOwnerAuthorityPda] = this.findAuthority(params.walletPda, credHash);

    const currentOwnerAuthorityPda = params.currentOwnerAuthorityPda
      ?? this.findAuthority(params.walletPda, params.ownerSigner.credentialIdHash)[0];
    const slot = params.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(currentOwnerAuthorityPda)) + 1;

    // On-chain: data_payload = [auth_type(1)][full_auth_data]
    // signed_payload = data_payload + payer.key()
    const dataPayload = buildDataPayloadForTransfer(
      params.newType, params.newCredentialOrPubkey,
      params.newSecp256r1Pubkey, params.newRpId,
    );
    const signedPayload = concatParts([dataPayload, params.payer.toBytes()]);

    const { authPayload, precompileIx } = await this.signWithSecp256r1({
      signer: params.ownerSigner,
      discriminator: new Uint8Array([DISC_TRANSFER_OWNERSHIP]),
      signedPayload,
      sysvarIxIndex: SYSVAR_IX_INDEX_TRANSFER_OWNERSHIP,
      slot,
      counter,
      payer: params.payer,
    });

    const ix = createTransferOwnershipIx({
      payer: params.payer,
      walletPda: params.walletPda,
      currentOwnerAuthorityPda,
      newOwnerAuthorityPda,
      newType: params.newType,
      credentialOrPubkey: params.newCredentialOrPubkey,
      secp256r1Pubkey: params.newSecp256r1Pubkey,
      rpId: params.newRpId,
      authPayload,
      programId: this.programId,
    });

    return { ix, newOwnerAuthorityPda, precompileIx };
  }

  // ─── Authorize (Deferred Execution tx1, Secp256r1) ───────────────
  async authorizeSecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    /** Auto-derived from signer.credentialIdHash if omitted */
    authorityPda?: PublicKey;
    signer: Secp256r1Signer;
    compactInstructions: CompactInstruction[];
    /** Account metas for the tx2 (ExecuteDeferred) layout */
    tx2AccountMetas: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
    /** Expiry offset in slots (default 300 = ~2 minutes) */
    expiryOffset?: number;
    /** Override slot (auto-fetched from connection if omitted) */
    slotOverride?: bigint;
  }): Promise<{
    authorizeIx: TransactionInstruction;
    precompileIx: TransactionInstruction;
    deferredExecPda: PublicKey;
    counter: number;
  }> {
    const authorityPda = params.authorityPda
      ?? this.findAuthority(params.walletPda, params.signer.credentialIdHash)[0];
    const slot = params.slotOverride ?? BigInt(await this.connection.getSlot());
    const counter = (await this.readCounter(authorityPda)) + 1;
    const expiryOffset = params.expiryOffset ?? 300;

    // Compute instruction hash and accounts hash for tx2
    const instructionsHash = computeInstructionsHash(params.compactInstructions);
    const accountsHash = computeAccountsHash(params.tx2AccountMetas, params.compactInstructions);

    // On-chain: signed_payload = instructions_hash || accounts_hash
    const signedPayload = concatParts([instructionsHash, accountsHash]);

    const { authPayload, precompileIx } = await this.signWithSecp256r1({
      signer: params.signer,
      discriminator: new Uint8Array([DISC_AUTHORIZE]),
      signedPayload,
      sysvarIxIndex: SYSVAR_IX_INDEX_AUTHORIZE,
      slot,
      counter,
      payer: params.payer,
    });

    // Derive DeferredExec PDA using the counter value
    const [deferredExecPda] = this.findDeferredExec(
      params.walletPda,
      authorityPda,
      counter,
    );

    const authorizeIx = createAuthorizeIx({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityPda,
      deferredExecPda,
      instructionsHash,
      accountsHash,
      expiryOffset,
      authPayload,
      programId: this.programId,
    });

    return { authorizeIx, precompileIx, deferredExecPda, counter };
  }

  // ─── ExecuteDeferred (Deferred Execution tx2) ───────────────────
  executeDeferred(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    deferredExecPda: PublicKey;
    refundDestination?: PublicKey;
    compactInstructions: CompactInstruction[];
    remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  }): TransactionInstruction {
    const [vaultPda] = this.findVault(params.walletPda);
    const refundDest = params.refundDestination ?? params.payer;
    const packed = packCompactInstructions(params.compactInstructions);
    return createExecuteDeferredIx({
      payer: params.payer,
      walletPda: params.walletPda,
      vaultPda,
      deferredExecPda: params.deferredExecPda,
      refundDestination: refundDest,
      packedInstructions: packed,
      remainingAccounts: params.remainingAccounts,
      programId: this.programId,
    });
  }

  // ─── ReclaimDeferred ────────────────────────────────────────────
  reclaimDeferred(params: {
    payer: PublicKey;
    deferredExecPda: PublicKey;
    refundDestination?: PublicKey;
  }): TransactionInstruction {
    const refundDest = params.refundDestination ?? params.payer;
    return createReclaimDeferredIx({
      payer: params.payer,
      deferredExecPda: params.deferredExecPda,
      refundDestination: refundDest,
      programId: this.programId,
    });
  }
}

// ─── Internal helpers ────────────────────────────────────────────────

/**
 * Converts standard Solana TransactionInstructions into the compact format
 * expected by the Execute instruction. Automatically computes account indexes
 * and builds the remaining accounts list.
 *
 * @param fixedAccounts - Accounts already in the Execute instruction layout
 *                        (payer, wallet, authority, vault, sysvar_instructions)
 * @param instructions  - Standard Solana TransactionInstructions to convert
 */
function buildCompactLayout(
  fixedAccounts: PublicKey[],
  instructions: TransactionInstruction[],
): {
  compactInstructions: CompactInstruction[];
  remainingAccounts: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
} {
  // Index map: pubkey base58 -> index in the full account layout
  const indexMap = new Map<string, number>();
  for (let i = 0; i < fixedAccounts.length; i++) {
    indexMap.set(fixedAccounts[i].toBase58(), i);
  }

  // Collect remaining accounts (unique, preserving insertion order)
  const remainingMap = new Map<string, { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }>();

  for (const ix of instructions) {
    // Program ID (never signer, never writable)
    const progKey = ix.programId.toBase58();
    if (!indexMap.has(progKey) && !remainingMap.has(progKey)) {
      remainingMap.set(progKey, { pubkey: ix.programId, isSigner: false, isWritable: false });
    }
    // Account keys
    for (const key of ix.keys) {
      const k = key.pubkey.toBase58();
      if (!indexMap.has(k)) {
        if (remainingMap.has(k)) {
          // Merge: most permissive flags win
          const existing = remainingMap.get(k)!;
          existing.isSigner = existing.isSigner || key.isSigner;
          existing.isWritable = existing.isWritable || key.isWritable;
        } else {
          remainingMap.set(k, { pubkey: key.pubkey, isSigner: key.isSigner, isWritable: key.isWritable });
        }
      }
    }
  }

  // Assign indexes to remaining accounts (after fixed accounts)
  const remainingAccounts = Array.from(remainingMap.values());
  let nextIndex = fixedAccounts.length;
  for (const acc of remainingAccounts) {
    indexMap.set(acc.pubkey.toBase58(), nextIndex++);
  }

  // Convert each instruction to compact format
  const compactInstructions: CompactInstruction[] = instructions.map(ix => ({
    programIdIndex: indexMap.get(ix.programId.toBase58())!,
    accountIndexes: ix.keys.map(k => indexMap.get(k.pubkey.toBase58())!),
    data: new Uint8Array(ix.data),
  }));

  return { compactInstructions, remainingAccounts };
}

/**
 * Builds data_payload for AddAuthority:
 * [type(1)][role(1)][padding(6)][credential(32)][secp256r1Pubkey?(33)][rpIdLen?(1)][rpId?(N)]
 *
 * Must match on-chain instruction_data[0..8+full_auth_data.len()]
 */
function buildDataPayloadForAdd(
  newType: number,
  newRole: number,
  credentialOrPubkey: Uint8Array,
  secp256r1Pubkey?: Uint8Array,
  rpId?: string,
): Uint8Array {
  const parts: Uint8Array[] = [
    new Uint8Array([newType, newRole]),
    new Uint8Array(6), // padding
    credentialOrPubkey,
  ];
  if (newType === AUTH_TYPE_SECP256R1 && secp256r1Pubkey) {
    parts.push(secp256r1Pubkey);
    if (rpId) {
      const rpIdBytes = Buffer.from(rpId, 'utf-8');
      parts.push(new Uint8Array([rpIdBytes.length]));
      parts.push(new Uint8Array(rpIdBytes));
    }
  }
  return concatParts(parts);
}

/**
 * Builds data_payload for TransferOwnership: [auth_type(1)][full_auth_data]
 * Matches on-chain instruction_data[0..data_payload_len]
 */
function buildDataPayloadForTransfer(
  newType: number,
  credentialOrPubkey: Uint8Array,
  secp256r1Pubkey?: Uint8Array,
  rpId?: string,
): Uint8Array {
  const parts: Uint8Array[] = [
    new Uint8Array([newType]),
    credentialOrPubkey,
  ];
  if (newType === AUTH_TYPE_SECP256R1 && secp256r1Pubkey) {
    parts.push(secp256r1Pubkey);
    if (rpId) {
      const rpIdBytes = Buffer.from(rpId, 'utf-8');
      parts.push(new Uint8Array([rpIdBytes.length]));
      parts.push(new Uint8Array(rpIdBytes));
    }
  }
  return concatParts(parts);
}

/**
 * Builds data_payload for CreateSession: [session_key(32)][expires_at(8)]
 * Matches on-chain CreateSessionArgs layout
 */
function buildDataPayloadForSession(
  sessionKey: Uint8Array,
  expiresAt: bigint,
): Uint8Array {
  const buf = new Uint8Array(40);
  buf.set(sessionKey, 0);
  const expiresAtBuf = Buffer.alloc(8);
  expiresAtBuf.writeBigInt64LE(expiresAt);
  buf.set(new Uint8Array(expiresAtBuf), 32);
  return buf;
}

function concatParts(parts: Uint8Array[]): Uint8Array {
  const totalLen = parts.reduce((s, a) => s + a.length, 0);
  const out = new Uint8Array(totalLen);
  let offset = 0;
  for (const a of parts) {
    out.set(a, offset);
    offset += a.length;
  }
  return out;
}

/**
 * Builds the Secp256r1 precompile verify instruction.
 * Program: Secp256r1SigVerify111111111111111111111111111
 *
 * Layout (must match on-chain introspection.rs constants):
 *   [num_signatures(1)][padding(1)]
 *   [sig_offset(2)][sig_ix_index(2)]
 *   [pubkey_offset(2)][pubkey_ix_index(2)]
 *   [msg_offset(2)][msg_size(2)][msg_ix_index(2)]
 *   [signature(64)][pubkey(33)][padding(1)][message(N)]
 *
 * Offsets: header=16, sig=16, pubkey=80, msg=114 (80+33+1 alignment)
 */
function buildSecp256r1PrecompileIx(
  publicKey: Uint8Array,
  message: Uint8Array,
  signature: Uint8Array,
): TransactionInstruction {
  const SECP256R1_PROGRAM_ID = new PublicKey('Secp256r1SigVerify1111111111111111111111111');

  const HEADER_SIZE = 16;
  const sigOffset = HEADER_SIZE;            // 16
  const pubkeyOffset = sigOffset + 64;      // 80
  const msgOffset = pubkeyOffset + 33 + 1;  // 114 (1-byte alignment padding)

  const data = Buffer.alloc(HEADER_SIZE + 64 + 33 + 1 + message.length);
  let off = 0;

  // Header
  data.writeUInt8(1, off); off += 1;              // num_signatures
  data.writeUInt8(0, off); off += 1;              // padding
  data.writeUInt16LE(sigOffset, off); off += 2;   // sig_offset
  data.writeUInt16LE(0xFFFF, off); off += 2;      // sig_ix_index (same tx)
  data.writeUInt16LE(pubkeyOffset, off); off += 2; // pubkey_offset
  data.writeUInt16LE(0xFFFF, off); off += 2;      // pubkey_ix_index
  data.writeUInt16LE(msgOffset, off); off += 2;   // msg_offset
  data.writeUInt16LE(message.length, off); off += 2; // msg_size
  data.writeUInt16LE(0xFFFF, off); off += 2;      // msg_ix_index

  // Payload
  Buffer.from(signature).copy(data, sigOffset);
  Buffer.from(publicKey).copy(data, pubkeyOffset);
  // Byte at offset 113 is alignment padding (zero)
  Buffer.from(message).copy(data, msgOffset);

  return new TransactionInstruction({
    programId: SECP256R1_PROGRAM_ID,
    keys: [],
    data,
  });
}
