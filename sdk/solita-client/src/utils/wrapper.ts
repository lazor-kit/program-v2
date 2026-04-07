import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  type AccountMeta,
} from '@solana/web3.js';

import { LazorInstructionBuilder } from './client';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findConfigPda,
  findTreasuryShardPda,
  findSessionPda,
  PROGRAM_ID,
} from './pdas';

import { Role } from '../generated';
import { AuthorityAccount } from '../generated/accounts';
import {
  type Secp256r1Signer,
  buildSecp256r1Message,
  buildSecp256r1PrecompileIx,
  appendSecp256r1Sysvars,
  buildAuthPayload,
  buildAuthenticatorData,
  readCurrentSlot,
} from './secp256r1';
import {
  computeAccountsHash,
  packCompactInstructions,
  type CompactInstruction,
} from './packing';
import bs58 from 'bs58';

// ─── Enums ───────────────────────────────────────────────────────────────────

export enum AuthType {
  Ed25519 = 0,
  Secp256r1 = 1,
}

export { Role };

// ─── Constants ───────────────────────────────────────────────────────────────

export const AUTHORITY_ACCOUNT_HEADER_SIZE = 48;
export const AUTHORITY_ACCOUNT_ED25519_SIZE = 48 + 32; // 80 bytes
export const AUTHORITY_ACCOUNT_SECP256R1_SIZE = 48 + 65; // 113 bytes
export const DISCRIMINATOR_AUTHORITY = 2;

// ─── Types ───────────────────────────────────────────────────────────────────

/**
 * Parameters for creating a new LazorKit wallet.
 *
 * - `userSeed` is optional — the SDK auto-generates a random 32-byte seed if omitted.
 * - For Ed25519: only `owner` public key is required.
 * - For Secp256r1: `pubkey` (33-byte compressed P-256 key) and `credentialHash` (32-byte SHA-256 of credential ID) are required.
 */
export type CreateWalletParams =
  | {
      payer: Keypair;
      authType: AuthType.Ed25519;
      owner: PublicKey;
      userSeed?: Uint8Array;
    }
  | {
      payer: Keypair;
      authType?: AuthType.Secp256r1;
      /** 33-byte compressed P-256 public key */
      pubkey: Uint8Array;
      /** 32-byte SHA-256 hash of the WebAuthn credential ID */
      credentialHash: Uint8Array;
      userSeed?: Uint8Array;
    };

/**
 * Identifies the admin/owner who is authorizing a privileged action.
 *
 * - Ed25519: provide `adminSigner` Keypair — SDK derives the Authority PDA automatically.
 * - Secp256r1: provide `adminCredentialHash` to derive the Authority PDA.
 *   The actual signature verification is done via a preceding Secp256r1 precompile instruction.
 */
export type AdminSignerOptions =
  | {
      adminType: AuthType.Ed25519;
      adminSigner: Keypair;
    }
  | {
      adminType?: AuthType.Secp256r1;
      /**
       * 32-byte SHA-256 hash of the WebAuthn credential ID.
       * Used to derive the admin Authority PDA.
       * The actual Secp256r1 signature must be provided as a separate
       * precompile instruction prepended to the transaction.
       */
      adminCredentialHash: Uint8Array;
    };

// ─── LazorClient ─────────────────────────────────────────────────────────────

/**
 * High-level client for the LazorKit Smart Wallet program.
 *
 * Two usage layers:
 *
 * **Layer 1 — Instruction builders** (`createWallet`, `addAuthority`, `execute`, …)
 *   Return a `TransactionInstruction` (plus any derived addresses).
 *   Callers compose multiple instructions and send the transaction themselves.
 *
 * **Layer 2 — Transaction builders** (`createWalletTxn`, `addAuthorityTxn`, …)
 *   Wrap Layer-1 results in a `Transaction` object ready to be signed and sent.
 *
 * Sending is always the caller's responsibility — use `connection.sendRawTransaction` or
 * any helper you prefer so that this SDK stays free of transport assumptions.
 */
export class LazorClient {
  public builder: LazorInstructionBuilder;

  constructor(
    public connection: Connection,
    public programId: PublicKey = PROGRAM_ID,
  ) {
    this.builder = new LazorInstructionBuilder(programId);
  }

  // ─── PDA helpers (instance convenience) ──────────────────────────────────

  /** Derives the Wallet PDA from a 32-byte seed. */
  getWalletPda(userSeed: Uint8Array): PublicKey {
    return findWalletPda(userSeed, this.programId)[0];
  }

  /** Derives the Vault PDA from a Wallet PDA. */
  getVaultPda(walletPda: PublicKey): PublicKey {
    return findVaultPda(walletPda, this.programId)[0];
  }

  /**
   * Derives an Authority PDA.
   * @param idSeed For Ed25519: 32-byte public key. For Secp256r1: 32-byte credential hash.
   */
  getAuthorityPda(
    walletPda: PublicKey,
    idSeed: Uint8Array | PublicKey,
  ): PublicKey {
    const seed = idSeed instanceof PublicKey ? idSeed.toBytes() : idSeed;
    return findAuthorityPda(walletPda, seed, this.programId)[0];
  }

  /** Derives a Session PDA from a wallet PDA and session public key. */
  getSessionPda(walletPda: PublicKey, sessionKey: PublicKey): PublicKey {
    return findSessionPda(walletPda, sessionKey, this.programId)[0];
  }

  /** Derives the global Config PDA. */
  getConfigPda(): PublicKey {
    return findConfigPda(this.programId)[0];
  }

  /** Derives a Treasury Shard PDA for a given shard index. */
  getTreasuryShardPda(shardId: number): PublicKey {
    return findTreasuryShardPda(shardId, this.programId)[0];
  }

  // ─── Internal helpers ─────────────────────────────────────────────────────

  private computeShardId(pubkey: PublicKey, numShards: number): number {
    const n = Math.max(1, Math.min(255, Math.floor(numShards)));
    return pubkey.toBytes().reduce((a, b) => a + b, 0) % n;
  }

  private async getConfigNumShards(configPda: PublicKey): Promise<number> {
    const info = await this.connection.getAccountInfo(configPda, 'confirmed');
    if (!info?.data || info.data.length < 4) return 16;

    // ConfigAccount layout (program/src/state/config.rs):
    // [0]=discriminator, [1]=bump, [2]=version, [3]=num_shards
    const discriminator = info.data[0];
    const numShards = info.data[3];

    // Discriminator 4 = Config. If this is not a Config PDA, fall back to default.
    if (discriminator !== 4) return 16;
    if (numShards === 0) return 16;
    return numShards;
  }

  private async getCommonPdas(
    payerPubkey: PublicKey,
  ): Promise<{ configPda: PublicKey; treasuryShard: PublicKey }> {
    const configPda = findConfigPda(this.programId)[0];
    const numShards = await this.getConfigNumShards(configPda);
    const shardId = this.computeShardId(payerPubkey, numShards);
    const treasuryShard = findTreasuryShardPda(shardId, this.programId)[0];
    return { configPda, treasuryShard };
  }

  // ─── Layer 1: Instruction builders ───────────────────────────────────────

  /**
   * Builds a `CreateWallet` instruction.
   *
   * - `userSeed` is optional — a random 32-byte seed is generated when omitted.
   * - Returns the derived `walletPda`, `authorityPda`, and the actual `userSeed` used,
   *   so callers can store the seed for later recovery.
   */
  async createWallet(params: CreateWalletParams): Promise<{
    ix: TransactionInstruction;
    walletPda: PublicKey;
    authorityPda: PublicKey;
    userSeed: Uint8Array;
  }> {
    const userSeed =
      params.userSeed ?? crypto.getRandomValues(new Uint8Array(32));
    const [walletPda] = findWalletPda(userSeed, this.programId);
    const [vaultPda] = findVaultPda(walletPda, this.programId);

    const authType = params.authType ?? AuthType.Secp256r1;
    let authorityPda: PublicKey;
    let authBump: number;
    let authPubkey: Uint8Array;
    let credentialHash: Uint8Array = new Uint8Array(32);

    if (params.authType === AuthType.Ed25519) {
      authPubkey = params.owner.toBytes();
      [authorityPda, authBump] = findAuthorityPda(
        walletPda,
        authPubkey,
        this.programId,
      );
    } else {
      const p = params as { pubkey: Uint8Array; credentialHash: Uint8Array };
      authPubkey = p.pubkey;
      credentialHash = p.credentialHash;
      [authorityPda, authBump] = findAuthorityPda(
        walletPda,
        credentialHash,
        this.programId,
      );
    }

    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );

    const ix = this.builder.createWallet({
      config: configPda,
      treasuryShard,
      payer: params.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: authorityPda,
      userSeed,
      authType,
      authBump,
      authPubkey,
      credentialHash,
    });

    return { ix, walletPda, authorityPda, userSeed };
  }

  /**
   * Builds an `AddAuthority` instruction.
   *
   * - `role` defaults to `Role.Spender`.
   * - `authType` defaults to `AuthType.Secp256r1`.
   * - `adminAuthorityPda` can be provided to override auto-derivation.
   */
  async addAuthority(
    params: {
      payer: Keypair;
      walletPda: PublicKey;
      newAuthType?: AuthType;
      newAuthPubkey: Uint8Array;
      newCredentialHash?: Uint8Array;
      role?: Role;
      /** Override the admin Authority PDA instead of auto-deriving it. */
      adminAuthorityPda?: PublicKey;
    } & AdminSignerOptions,
  ): Promise<{ ix: TransactionInstruction; newAuthority: PublicKey }> {
    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );

    const newAuthType = params.newAuthType ?? AuthType.Secp256r1;
    const role = params.role ?? Role.Spender;
    const adminType = params.adminType ?? AuthType.Secp256r1;

    const idSeed =
      newAuthType === AuthType.Secp256r1
        ? params.newCredentialHash ?? new Uint8Array(32)
        : params.newAuthPubkey.slice(0, 32);
    const [newAuthority] = findAuthorityPda(
      params.walletPda,
      idSeed,
      this.programId,
    );

    let adminAuthority: PublicKey;
    if (params.adminAuthorityPda) {
      adminAuthority = params.adminAuthorityPda;
    } else if (adminType === AuthType.Ed25519) {
      const p = params as { adminSigner: Keypair };
      [adminAuthority] = findAuthorityPda(
        params.walletPda,
        p.adminSigner.publicKey.toBytes(),
        this.programId,
      );
    } else {
      const p = params as { adminCredentialHash: Uint8Array };
      [adminAuthority] = findAuthorityPda(
        params.walletPda,
        p.adminCredentialHash,
        this.programId,
      );
    }

    const ix = this.builder.addAuthority({
      payer: params.payer.publicKey,
      wallet: params.walletPda,
      adminAuthority,
      newAuthority,
      config: configPda,
      treasuryShard,
      newAuthType,
      newRole: role,
      newAuthPubkey: params.newAuthPubkey,
      newCredentialHash: params.newCredentialHash,
      authorizerSigner:
        adminType === AuthType.Ed25519
          ? (params as any).adminSigner.publicKey
          : undefined,
    });

    return { ix, newAuthority };
  }

  /**
   * Builds a `RemoveAuthority` instruction.
   *
   * - `refundDestination` is optional — defaults to `payer.publicKey`.
   */
  async removeAuthority(
    params: {
      payer: Keypair;
      walletPda: PublicKey;
      authorityToRemovePda: PublicKey;
      /** Where to send the recovered rent SOL. Defaults to payer. */
      refundDestination?: PublicKey;
    } & AdminSignerOptions,
  ): Promise<TransactionInstruction> {
    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );
    const refundDestination =
      params.refundDestination ?? params.payer.publicKey;

    let adminAuthority: PublicKey;
    if (params.adminType === AuthType.Ed25519) {
      [adminAuthority] = findAuthorityPda(
        params.walletPda,
        params.adminSigner.publicKey.toBytes(),
        this.programId,
      );
    } else {
      [adminAuthority] = findAuthorityPda(
        params.walletPda,
        params.adminCredentialHash,
        this.programId,
      );
    }

    return this.builder.removeAuthority({
      config: configPda,
      treasuryShard,
      payer: params.payer.publicKey,
      wallet: params.walletPda,
      adminAuthority,
      targetAuthority: params.authorityToRemovePda,
      refundDestination,
      authorizerSigner:
        params.adminType === AuthType.Ed25519
          ? params.adminSigner.publicKey
          : undefined,
    });
  }

  /**
   * Builds a `CreateSession` instruction.
   *
   * - `sessionKey` is optional — an ephemeral Keypair is auto-generated when omitted.
   * - `expiresAt` is optional — defaults to 1 hour from now (in Unix seconds).
   * - Returns the generated `sessionKeypair` so the caller can store / use it.
   */
  async createSession(
    params: {
      payer: Keypair;
      walletPda: PublicKey;
      /** Session public key. Omit to let the SDK auto-generate an ephemeral keypair. */
      sessionKey?: PublicKey;
      /**
       * Absolute slot height (or Unix timestamp) at which the session expires.
       * Defaults to `Date.now() / 1000 + 3600` (1 hour from now).
       */
      expiresAt?: bigint | number;
    } & AdminSignerOptions,
  ): Promise<{
    ix: TransactionInstruction;
    sessionPda: PublicKey;
    /** The session keypair — only set when auto-generated; null if caller supplied sessionKey. */
    sessionKeypair: Keypair | null;
  }> {
    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );
    const expiresAt =
      params.expiresAt != null
        ? BigInt(params.expiresAt)
        : BigInt(Math.floor(Date.now() / 1000) + 3600);
    const adminType = params.adminType ?? AuthType.Secp256r1;

    let sessionKeypair: Keypair | null = null;
    let sessionKeyPubkey: PublicKey;
    if (params.sessionKey) {
      sessionKeyPubkey = params.sessionKey;
    } else {
      sessionKeypair = Keypair.generate();
      sessionKeyPubkey = sessionKeypair.publicKey;
    }

    const [sessionPda] = findSessionPda(
      params.walletPda,
      sessionKeyPubkey,
      this.programId,
    );

    let adminAuthority: PublicKey;
    if (adminType === AuthType.Ed25519) {
      const p = params as { adminSigner: Keypair };
      [adminAuthority] = findAuthorityPda(
        params.walletPda,
        p.adminSigner.publicKey.toBytes(),
        this.programId,
      );
    } else {
      const p = params as { adminCredentialHash: Uint8Array };
      [adminAuthority] = findAuthorityPda(
        params.walletPda,
        p.adminCredentialHash,
        this.programId,
      );
    }

    const ix = this.builder.createSession({
      config: configPda,
      treasuryShard,
      payer: params.payer.publicKey,
      wallet: params.walletPda,
      adminAuthority,
      session: sessionPda,
      sessionKey: Array.from(sessionKeyPubkey.toBytes()),
      expiresAt,
      authorizerSigner:
        adminType === AuthType.Ed25519
          ? (params as any).adminSigner.publicKey
          : undefined,
    });

    return { ix, sessionPda, sessionKeypair };
  }

  /**
   * Builds a `CloseSession` instruction.
   */
  async closeSession(params: {
    payer: Keypair;
    walletPda: PublicKey;
    sessionPda: PublicKey;
    /** Override the Config PDA if needed. */
    configPda?: PublicKey;
    authorizer?: {
      authorizerPda: PublicKey;
      signer: Keypair;
    };
  }): Promise<TransactionInstruction> {
    const configPda = params.configPda ?? findConfigPda(this.programId)[0];

    return this.builder.closeSession({
      config: configPda,
      payer: params.payer.publicKey,
      wallet: params.walletPda,
      session: params.sessionPda,
      authorizer: params.authorizer?.authorizerPda,
      authorizerSigner: params.authorizer?.signer.publicKey,
    });
  }

  /**
   * Builds an `Execute` instruction using the high-level builder that handles
   * account deduplication and CompactInstruction packing automatically.
   *
   * - `authorityPda` is optional when `signer` (Ed25519 Keypair) is provided —
   *   the SDK derives the Authority PDA from `signer.publicKey`.
   * - `vaultPda` is optional — derived from `walletPda` when omitted.
   */
  async execute(params: {
    payer: Keypair;
    walletPda: PublicKey;
    innerInstructions: TransactionInstruction[];
    /** Authority PDA. Omit when `signer` is an Ed25519 Keypair — SDK auto-derives it. */
    authorityPda?: PublicKey;
    /** Ed25519 keypair that signs the transaction (authorizer signer). */
    signer?: Keypair;
    /** Secp256r1 signature bytes appended to instruction data. */
    signature?: Uint8Array;
    /** Override vault PDA if different from the canonical derivation. */
    vaultPda?: PublicKey;
  }): Promise<TransactionInstruction> {
    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );
    const vaultPda =
      params.vaultPda ?? findVaultPda(params.walletPda, this.programId)[0];

    // Auto-derive authorityPda from signer if not provided
    let authorityPda = params.authorityPda;
    if (!authorityPda && params.signer) {
      [authorityPda] = findAuthorityPda(
        params.walletPda,
        params.signer.publicKey.toBytes(),
        this.programId,
      );
    }
    if (!authorityPda) {
      throw new Error(
        'execute(): either `authorityPda` or `signer` must be provided so the SDK can identify the Authority PDA.',
      );
    }

    const ix = this.builder.buildExecute({
      config: configPda,
      treasuryShard,
      payer: params.payer.publicKey,
      wallet: params.walletPda,
      authority: authorityPda,
      vault: vaultPda,
      innerInstructions: params.innerInstructions,
      authorizerSigner: params.signer?.publicKey,
    });

    if (params.signature) {
      const newData = Buffer.alloc(ix.data.length + params.signature.length);
      ix.data.copy(newData);
      newData.set(params.signature, ix.data.length);
      ix.data = newData;
    }

    return ix;
  }

  /**
   * Builds a bundle of two instructions for Secp256r1 Execution:
   * 1. Secp256r1 Precompile Instruction
   * 2. LazorKit Execute Instruction with appended signature payload
   *
   * @returns { precompileIx, executeIx } object containing both instructions
   */
  async executeWithSecp256r1(params: {
    payer: Keypair;
    walletPda: PublicKey;
    innerInstructions: TransactionInstruction[];
    signer: Secp256r1Signer;
    /** Optional: custom authenticatorData bytes. If omitted, counter is fetched from Authority account. */
    authenticatorData?: Uint8Array;
    /** Optional: absolute slot height for liveness proof. fetched automatically if 0n/omitted */
    slot?: bigint;
    rpId?: string;
    /** Optional: Fully qualified origin URL like "https://my-app.com" */
    origin?: string;
  }): Promise<{
    precompileIx: TransactionInstruction;
    executeIx: TransactionInstruction;
  }> {
    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );
    const vaultPda = findVaultPda(params.walletPda, this.programId)[0];
    const [authorityPda] = findAuthorityPda(
      params.walletPda,
      params.signer.credentialIdHash,
      this.programId,
    );

    // 1. Replicate deduplication to get exact account list for hashing
    const baseAccounts: PublicKey[] = [
      params.payer.publicKey,
      params.walletPda,
      authorityPda,
      vaultPda,
      configPda,
      treasuryShard,
      SystemProgram.programId,
    ];

    const accountMap = new Map<string, number>();
    const accountMetas: AccountMeta[] = [];

    baseAccounts.forEach((pk, idx) => {
      accountMap.set(pk.toBase58(), idx);
      accountMetas.push({
        pubkey: pk,
        isWritable: idx === 0 || idx === 2 || idx === 3 || idx === 5,
        isSigner: idx === 0,
      });
    });

    const vaultKey = vaultPda.toBase58();
    const walletKey = params.walletPda.toBase58();

    const addAccount = (
      pubkey: PublicKey,
      isSigner: boolean,
      isWritable: boolean,
    ): number => {
      const key = pubkey.toBase58();
      if (key === vaultKey || key === walletKey) isSigner = false;

      if (!accountMap.has(key)) {
        const idx = accountMetas.length;
        accountMap.set(key, idx);
        accountMetas.push({ pubkey, isWritable, isSigner });
        return idx;
      } else {
        const idx = accountMap.get(key)!;
        const existing = accountMetas[idx];
        if (isWritable) existing.isWritable = true;
        if (isSigner) existing.isSigner = true;
        return idx;
      }
    };

    const compactIxs: CompactInstruction[] = [];
    for (const ix of params.innerInstructions) {
      const programIdIndex = addAccount(ix.programId, false, false);
      const accountIndexes: number[] = [];
      for (const acc of ix.keys) {
        accountIndexes.push(
          addAccount(acc.pubkey, acc.isSigner, acc.isWritable),
        );
      }
      compactIxs.push({ programIdIndex, accountIndexes, data: ix.data });
    }

    const packedInstructions = packCompactInstructions(compactIxs);

    // 2. Load Slot
    const slot = params.slot ?? (await readCurrentSlot(this.connection));

    // 2b. Fetch current counter from Authority account if not using custom authenticatorData
    let counter = 1;
    if (!params.authenticatorData) {
      try {
        const authAccount = await AuthorityAccount.fromAccountAddress(
          this.connection,
          authorityPda,
        );
        counter = Number(authAccount.counter) + 1;
      } catch {
        // If Authority doesn't exist yet, default to 1
        counter = 1;
      }
    }

    // 3. Build Auth Payload
    const executeIxBase = new TransactionInstruction({
      programId: this.programId,
      keys: [...accountMetas], // copy
      data: Buffer.alloc(0),
    });
    const {
      ix: ixWithSysvars,
      sysvarIxIndex,
      sysvarSlotIndex,
    } = appendSecp256r1Sysvars(executeIxBase);

    const authenticatorData =
      params.authenticatorData ??
      (await buildAuthenticatorData(params.rpId, counter));

    const authPayload = buildAuthPayload({
      sysvarIxIndex,
      sysvarSlotIndex,
      authenticatorData,
      slot,
      rpId: params.rpId,
    });

    // 4. Compute Accounts Hash
    const accountsHash = await computeAccountsHash(
      ixWithSysvars.keys,
      compactIxs,
    );

    // 5. Build Combined Signed Payload = PackedInstructions + AccountsHash
    const signedPayload = new Uint8Array(packedInstructions.length + 32);
    signedPayload.set(packedInstructions, 0);
    signedPayload.set(accountsHash, packedInstructions.length);

    // 6. Generate Message to Sign
    const message = await buildSecp256r1Message({
      discriminator: 4, // Execute
      authPayload,
      signedPayload,
      payer: params.payer.publicKey,
      programId: this.programId,
      slot,
      origin: params.origin,
      rpId: params.rpId,
      counter,
    });

    // 7. Get Precompile Instruction
    const precompileIx = await buildSecp256r1PrecompileIx(
      params.signer,
      message,
    );

    // 8. Build final Execute Instruction Data
    // Layout: [disc(1)] [packedInstructions] [authPayload]
    // Program parses: data_payload = &instruction_data[..compact_len] (the packed instructions)
    //                 authority_payload = &instruction_data[compact_len..] (the auth payload)
    const finalData = Buffer.alloc(
      1 + packedInstructions.length + authPayload.length,
    );
    finalData[0] = 4; // disc
    finalData.set(packedInstructions, 1);
    finalData.set(authPayload, 1 + packedInstructions.length);

    const executeIx = new TransactionInstruction({
      programId: this.programId,
      keys: ixWithSysvars.keys,
      data: finalData,
    });

    return { precompileIx, executeIx };
  }

  /**
   * AddAuthority authorized by a Secp256r1 admin.
   * Builds: 1) Secp precompile, 2) final AddAuthority ix with appended auth payload.
   */
  async addAuthorityWithSecp256r1(params: {
    payer: Keypair;
    walletPda: PublicKey;
    signer: Secp256r1Signer; // admin signer (has credentialIdHash/publicKeyBytes)
    newAuthType: AuthType;
    newAuthPubkey: Uint8Array;
    newCredentialHash?: Uint8Array;
    role: Role;
    slot?: bigint;
    rpId?: string;
    origin?: string;
  }): Promise<{
    precompileIx: TransactionInstruction;
    addIx: TransactionInstruction;
  }> {
    // 1) Build base ix (without auth payload)
    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );
    const { newAuthType, role } = params;
    const { walletPda } = params;

    const [adminAuthority] = findAuthorityPda(
      walletPda,
      params.signer.credentialIdHash,
      this.programId,
    );
    const [newAuthority] = findAuthorityPda(
      walletPda,
      newAuthType === AuthType.Secp256r1
        ? params.newCredentialHash ?? new Uint8Array(32)
        : params.newAuthPubkey.slice(0, 32),
      this.programId,
    );

    let addIx = this.builder.addAuthority({
      payer: params.payer.publicKey,
      wallet: walletPda,
      adminAuthority,
      newAuthority,
      config: configPda,
      treasuryShard,
      newAuthType,
      newRole: role,
      newAuthPubkey: params.newAuthPubkey,
      newCredentialHash: params.newCredentialHash,
    });

    // 2) Append sysvars and build auth payload
    const {
      ix: ixWithSysvars,
      sysvarIxIndex,
      sysvarSlotIndex,
    } = appendSecp256r1Sysvars(addIx);
    const slot = params.slot ?? (await readCurrentSlot(this.connection));
    // Fetch current counter from admin authority and use counter+1 for the signature
    let counter = 1;
    try {
      const authAccount = await AuthorityAccount.fromAccountAddress(
        this.connection,
        adminAuthority,
      );
      counter = Number(authAccount.counter) + 1;
    } catch {
      counter = 1;
    }
    const authenticatorData = await buildAuthenticatorData(
      params.rpId,
      counter,
    );
    const authPayload = buildAuthPayload({
      sysvarIxIndex,
      sysvarSlotIndex,
      authenticatorData,
      slot,
      rpId: params.rpId,
    });

    // 3) Build signed payload = data_payload (args+full_auth) + payer + wallet
    const argsAndFull = ixWithSysvars.data.subarray(1); // after disc(1)
    const extended = new Uint8Array(argsAndFull.length + 64);
    extended.set(argsAndFull, 0);
    extended.set(params.payer.publicKey.toBytes(), argsAndFull.length);
    extended.set(walletPda.toBytes(), argsAndFull.length + 32);

    const message = await buildSecp256r1Message({
      discriminator: 1,
      authPayload,
      signedPayload: extended,
      payer: params.payer.publicKey,
      programId: this.programId,
      slot,
      origin: params.origin,
      rpId: params.rpId,
      counter,
    });

    const precompileIx = await buildSecp256r1PrecompileIx(
      params.signer,
      message,
    );

    // 4) Final instruction data = [disc][args+full_auth][authPayload]
    const finalData = Buffer.alloc(1 + argsAndFull.length + authPayload.length);
    finalData[0] = 1;
    finalData.set(argsAndFull, 1);
    finalData.set(authPayload, 1 + argsAndFull.length);
    ixWithSysvars.data = finalData;

    return { precompileIx, addIx: ixWithSysvars };
  }

  /** CreateSession authorized by a Secp256r1 admin. */
  async createSessionWithSecp256r1(params: {
    payer: Keypair;
    walletPda: PublicKey;
    signer: Secp256r1Signer; // admin signer
    sessionKey?: PublicKey;
    expiresAt?: bigint | number;
    slot?: bigint;
    rpId?: string;
    origin?: string;
  }): Promise<{
    precompileIx: TransactionInstruction;
    sessionIx: TransactionInstruction;
    sessionPda: PublicKey;
    sessionKeypair: Keypair | null;
  }> {
    const expiresAt =
      params.expiresAt != null
        ? BigInt(params.expiresAt)
        : BigInt(Math.floor(Date.now() / 1000) + 3600);
    let sessionKeypair: Keypair | null = null;
    let sessionKeyPubkey: PublicKey;
    if (params.sessionKey) {
      sessionKeyPubkey = params.sessionKey;
    } else {
      sessionKeypair = Keypair.generate();
      sessionKeyPubkey = sessionKeypair.publicKey;
    }

    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );
    const [adminAuthority] = findAuthorityPda(
      params.walletPda,
      params.signer.credentialIdHash,
      this.programId,
    );
    const [sessionPda] = findSessionPda(
      params.walletPda,
      sessionKeyPubkey,
      this.programId,
    );

    let sessionIx = this.builder.createSession({
      payer: params.payer.publicKey,
      wallet: params.walletPda,
      adminAuthority,
      session: sessionPda,
      config: configPda,
      treasuryShard,
      systemProgram: undefined as any,
      sessionKey: Array.from(sessionKeyPubkey.toBytes()),
      expiresAt,
    } as any);

    const {
      ix: ixWithSysvars,
      sysvarIxIndex,
      sysvarSlotIndex,
    } = appendSecp256r1Sysvars(sessionIx);
    // Ensure admin authority is writable for Secp256r1 paths (program expects mutable borrow)
    for (const k of ixWithSysvars.keys) {
      if (k.pubkey.equals(adminAuthority)) {
        k.isWritable = true;
        break;
      }
    }
    const slot = params.slot ?? (await readCurrentSlot(this.connection));
    let counter = 1;
    try {
      const authAccount = await AuthorityAccount.fromAccountAddress(
        this.connection,
        adminAuthority,
      );
      counter = Number(authAccount.counter) + 1;
    } catch {
      counter = 1;
    }
    const authenticatorData = await buildAuthenticatorData(
      params.rpId,
      counter,
    );
    const authPayload = buildAuthPayload({
      sysvarIxIndex,
      sysvarSlotIndex,
      authenticatorData,
      slot,
      rpId: params.rpId,
    });

    // signed payload = args (after disc) + payer + wallet
    const args = ixWithSysvars.data.subarray(1);
    const extended = new Uint8Array(args.length + 64);
    extended.set(args, 0);
    extended.set(params.payer.publicKey.toBytes(), args.length);
    extended.set(params.walletPda.toBytes(), args.length + 32);

    const message = await buildSecp256r1Message({
      discriminator: 5,
      authPayload,
      signedPayload: extended,
      payer: params.payer.publicKey,
      programId: this.programId,
      slot,
      origin: params.origin,
      rpId: params.rpId,
      counter,
    });
    const precompileIx = await buildSecp256r1PrecompileIx(
      params.signer,
      message,
    );

    const finalData = Buffer.alloc(1 + args.length + authPayload.length);
    finalData[0] = 5;
    finalData.set(args, 1);
    finalData.set(authPayload, 1 + args.length);
    ixWithSysvars.data = finalData;

    return {
      precompileIx,
      sessionIx: ixWithSysvars,
      sessionPda,
      sessionKeypair,
    };
  }

  /** RemoveAuthority authorized by Secp256r1 admin. */
  async removeAuthorityWithSecp256r1(params: {
    payer: Keypair;
    walletPda: PublicKey;
    signer: Secp256r1Signer; // admin signer
    authorityToRemovePda: PublicKey;
    refundDestination?: PublicKey;
    slot?: bigint;
    rpId?: string;
    origin?: string;
  }): Promise<{
    precompileIx: TransactionInstruction;
    removeIx: TransactionInstruction;
  }> {
    const refundDestination =
      params.refundDestination ?? params.payer.publicKey;
    // base ix (no payload)
    let ix = await this.removeAuthority({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityToRemovePda: params.authorityToRemovePda,
      refundDestination,
      adminType: AuthType.Secp256r1,
      adminCredentialHash: params.signer.credentialIdHash,
    } as any);

    const {
      ix: ixWithSysvars,
      sysvarIxIndex,
      sysvarSlotIndex,
    } = appendSecp256r1Sysvars(ix);
    const slot = params.slot ?? (await readCurrentSlot(this.connection));
    let counter = 1;
    try {
      const authAccount = await AuthorityAccount.fromAccountAddress(
        this.connection,
        findAuthorityPda(
          params.walletPda,
          params.signer.credentialIdHash,
          this.programId,
        )[0],
      );
      counter = Number(authAccount.counter) + 1;
    } catch {
      counter = 1;
    }
    const authenticatorData = await buildAuthenticatorData(
      params.rpId,
      counter,
    );
    const authPayload = buildAuthPayload({
      sysvarIxIndex,
      sysvarSlotIndex,
      authenticatorData,
      slot,
      rpId: params.rpId,
    });

    // data_payload = target_auth_pda || refund_dest
    const target = params.authorityToRemovePda.toBytes();
    const refund = refundDestination.toBytes();
    const dataPayload = new Uint8Array(64);
    dataPayload.set(target, 0);
    dataPayload.set(refund, 32);

    const message = await buildSecp256r1Message({
      discriminator: 2,
      authPayload,
      signedPayload: dataPayload,
      payer: params.payer.publicKey,
      programId: this.programId,
      slot,
      origin: params.origin,
      rpId: params.rpId,
      counter,
    });
    const precompileIx = await buildSecp256r1PrecompileIx(
      params.signer,
      message,
    );

    // final data = [disc][authPayload]
    const finalData = Buffer.alloc(1 + authPayload.length);
    finalData[0] = 2;
    finalData.set(authPayload, 1);
    ixWithSysvars.data = finalData;

    return { precompileIx, removeIx: ixWithSysvars };
  }

  /** CloseSession authorized by Secp256r1 admin (Owner/Admin or contract admin if expired). */
  async closeSessionWithSecp256r1(params: {
    payer: Keypair;
    walletPda: PublicKey;
    signer: Secp256r1Signer; // admin signer
    sessionPda: PublicKey;
    authorizerPda?: PublicKey; // optional admin/owner authority PDA
    slot?: bigint;
    rpId?: string;
    origin?: string;
  }): Promise<{
    precompileIx: TransactionInstruction;
    closeIx: TransactionInstruction;
  }> {
    let ix = await this.closeSession({
      payer: params.payer,
      walletPda: params.walletPda,
      sessionPda: params.sessionPda,
      authorizer: params.authorizerPda
        ? ({
            authorizerPda: params.authorizerPda,
            signer: Keypair.generate(),
          } as any)
        : undefined,
    } as any);

    const {
      ix: ixWithSysvars,
      sysvarIxIndex,
      sysvarSlotIndex,
    } = appendSecp256r1Sysvars(ix);
    const slot = params.slot ?? (await readCurrentSlot(this.connection));
    let counter = 1;
    try {
      const authAccount = await AuthorityAccount.fromAccountAddress(
        this.connection,
        findAuthorityPda(
          params.walletPda,
          params.signer.credentialIdHash,
          this.programId,
        )[0],
      );
      counter = Number(authAccount.counter) + 1;
    } catch {
      counter = 1;
    }
    const authenticatorData = await buildAuthenticatorData(
      params.rpId,
      counter,
    );
    const authPayload = buildAuthPayload({
      sysvarIxIndex,
      sysvarSlotIndex,
      authenticatorData,
      slot,
      rpId: params.rpId,
    });

    const dataPayload = params.sessionPda.toBytes();
    const message = await buildSecp256r1Message({
      discriminator: 8,
      authPayload,
      signedPayload: dataPayload,
      payer: params.payer.publicKey,
      programId: this.programId,
      slot,
      origin: params.origin,
      rpId: params.rpId,
      counter,
    });
    const precompileIx = await buildSecp256r1PrecompileIx(
      params.signer,
      message,
    );

    const finalData = Buffer.alloc(1 + authPayload.length);
    finalData[0] = 8;
    finalData.set(authPayload, 1);
    ixWithSysvars.data = finalData;

    return { precompileIx, closeIx: ixWithSysvars };
  }

  /**
   * Builds a `CloseWallet` instruction.
   *
   * - `destination` is optional — defaults to `payer.publicKey`.
   * - `vaultPda` is optional — derived from `walletPda` when omitted.
   * - `adminAuthorityPda` is optional — auto-derived from admin signer credentials.
   */
  async closeWallet(
    params: {
      payer: Keypair;
      walletPda: PublicKey;
      /** Where to sweep all remaining SOL. Defaults to payer. */
      destination?: PublicKey;
      /** Override the Vault PDA if needed. */
      vaultPda?: PublicKey;
      /** Override the owner Authority PDA instead of auto-deriving it. */
      adminAuthorityPda?: PublicKey;
    } & AdminSignerOptions,
  ): Promise<TransactionInstruction> {
    const vaultPda =
      params.vaultPda ?? findVaultPda(params.walletPda, this.programId)[0];
    const destination = params.destination ?? params.payer.publicKey;

    let ownerAuthority: PublicKey;
    if (params.adminAuthorityPda) {
      ownerAuthority = params.adminAuthorityPda;
    } else if (params.adminType === AuthType.Ed25519) {
      [ownerAuthority] = findAuthorityPda(
        params.walletPda,
        params.adminSigner.publicKey.toBytes(),
        this.programId,
      );
    } else {
      [ownerAuthority] = findAuthorityPda(
        params.walletPda,
        params.adminCredentialHash ?? new Uint8Array(),
        this.programId,
      );
    }

    const ix = this.builder.closeWallet({
      payer: params.payer.publicKey,
      wallet: params.walletPda,
      vault: vaultPda,
      ownerAuthority,
      destination,
      ownerSigner:
        params.adminType === AuthType.Ed25519
          ? params.adminSigner.publicKey
          : undefined,
    });

    // Required by on-chain close logic
    ix.keys.push({
      pubkey: SystemProgram.programId,
      isWritable: false,
      isSigner: false,
    });

    return ix;
  }

  /**
   * Builds a `TransferOwnership` instruction.
   */
  async transferOwnership(params: {
    payer: Keypair;
    walletPda: PublicKey;
    currentOwnerAuthority: PublicKey;
    newOwnerAuthority: PublicKey;
    newAuthType: AuthType;
    newAuthPubkey: Uint8Array;
    newCredentialHash?: Uint8Array;
    /** Ed25519 signer (optional — for Secp256r1, auth comes via precompile instruction). */
    signer?: Keypair;
  }): Promise<TransactionInstruction> {
    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );

    return this.builder.transferOwnership({
      payer: params.payer.publicKey,
      wallet: params.walletPda,
      currentOwnerAuthority: params.currentOwnerAuthority,
      newOwnerAuthority: params.newOwnerAuthority,
      config: configPda,
      treasuryShard,
      newAuthType: params.newAuthType,
      newAuthPubkey: params.newAuthPubkey,
      newCredentialHash: params.newCredentialHash,
      authorizerSigner: params.signer?.publicKey,
    });
  }

  /**
   * TransferOwnership authorized by a Secp256r1 admin.
   * Builds: 1) Secp precompile, 2) final TransferOwnership ix with appended auth payload.
   */
  async transferOwnershipWithSecp256r1(params: {
    payer: Keypair;
    walletPda: PublicKey;
    signer: Secp256r1Signer; // admin signer
    newAuthPubkey: Uint8Array;
    newCredentialHash?: Uint8Array;
    newAuthType: AuthType;
    slot?: bigint;
    rpId?: string;
    origin?: string;
  }): Promise<{
    precompileIx: TransactionInstruction;
    transferIx: TransactionInstruction;
  }> {
    const { configPda, treasuryShard } = await this.getCommonPdas(
      params.payer.publicKey,
    );

    // derive current owner authority PDA from signer credentials
    const [currentOwnerAuthority] = findAuthorityPda(
      params.walletPda,
      params.signer.credentialIdHash,
      this.programId,
    );

    // derive new owner authority PDA depending on auth type
    const [newOwnerAuthority] = findAuthorityPda(
      params.walletPda,
      params.newAuthType === AuthType.Secp256r1
        ? params.newCredentialHash ?? new Uint8Array(32)
        : params.newAuthPubkey.slice(0, 32),
      this.programId,
    );

    // base ix (no auth payload)
    let ix = await this.transferOwnership({
      payer: params.payer,
      walletPda: params.walletPda,
      currentOwnerAuthority,
      newOwnerAuthority,
      newAuthType: params.newAuthType,
      newAuthPubkey: params.newAuthPubkey,
      newCredentialHash: params.newCredentialHash,
      config: configPda,
      treasuryShard,
    } as any);

    const {
      ix: ixWithSysvars,
      sysvarIxIndex,
      sysvarSlotIndex,
    } = appendSecp256r1Sysvars(ix);
    const slot = params.slot ?? (await readCurrentSlot(this.connection));
    const authenticatorData = await buildAuthenticatorData(params.rpId);
    const authPayload = buildAuthPayload({
      sysvarIxIndex,
      sysvarSlotIndex,
      authenticatorData,
      slot,
      rpId: params.rpId,
    });

    // signed payload = args (after disc) + payer + wallet
    const args = ixWithSysvars.data.subarray(1);
    const extended = new Uint8Array(args.length + 64);
    extended.set(args, 0);
    extended.set(params.payer.publicKey.toBytes(), args.length);
    extended.set(params.walletPda.toBytes(), args.length + 32);

    const message = await buildSecp256r1Message({
      discriminator: 3,
      authPayload,
      signedPayload: extended,
      payer: params.payer.publicKey,
      programId: this.programId,
      slot,
      origin: params.origin,
      rpId: params.rpId,
    });
    const precompileIx = await buildSecp256r1PrecompileIx(
      params.signer,
      message,
    );

    const finalData = Buffer.alloc(1 + args.length + authPayload.length);
    finalData[0] = 3;
    finalData.set(args, 1);
    finalData.set(authPayload, 1 + args.length);
    ixWithSysvars.data = finalData;

    return { precompileIx, transferIx: ixWithSysvars };
  }

  /**
   * CloseWallet authorized by a Secp256r1 admin.
   * Builds: 1) Secp precompile, 2) final CloseWallet ix with appended auth payload.
   */
  async closeWalletWithSecp256r1(params: {
    payer: Keypair;
    walletPda: PublicKey;
    signer: Secp256r1Signer;
    destination?: PublicKey;
    vaultPda?: PublicKey;
    adminAuthorityPda?: PublicKey;
    slot?: bigint;
    rpId?: string;
    origin?: string;
  }): Promise<{
    precompileIx: TransactionInstruction;
    closeIx: TransactionInstruction;
  }> {
    // Build base closeWallet ix using Secp admin credential to derive PDA
    let ix = await this.closeWallet({
      payer: params.payer,
      walletPda: params.walletPda,
      destination: params.destination,
      vaultPda: params.vaultPda,
      adminAuthorityPda: params.adminAuthorityPda,
      adminType: AuthType.Secp256r1,
      adminCredentialHash: params.signer.credentialIdHash,
    } as any);

    const {
      ix: ixWithSysvars,
      sysvarIxIndex,
      sysvarSlotIndex,
    } = appendSecp256r1Sysvars(ix);
    const slot = params.slot ?? (await readCurrentSlot(this.connection));
    const authenticatorData = await buildAuthenticatorData(params.rpId);
    const authPayload = buildAuthPayload({
      sysvarIxIndex,
      sysvarSlotIndex,
      authenticatorData,
      slot,
      rpId: params.rpId,
    });

    // signed payload = args (after disc) + payer + wallet
    const args = ixWithSysvars.data.subarray(1);
    const extended = new Uint8Array(args.length + 64);
    extended.set(args, 0);
    extended.set(params.payer.publicKey.toBytes(), args.length);
    extended.set(params.walletPda.toBytes(), args.length + 32);

    const message = await buildSecp256r1Message({
      discriminator: 9,
      authPayload,
      signedPayload: extended,
      payer: params.payer.publicKey,
      programId: this.programId,
      slot,
      origin: params.origin,
      rpId: params.rpId,
    });

    const precompileIx = await buildSecp256r1PrecompileIx(
      params.signer,
      message,
    );

    const finalData = Buffer.alloc(1 + args.length + authPayload.length);
    finalData[0] = 9;
    finalData.set(args, 1);
    finalData.set(authPayload, 1 + args.length);
    ixWithSysvars.data = finalData;

    return { precompileIx, closeIx: ixWithSysvars };
  }

  // ─── Admin instructions ───────────────────────────────────────────────────

  /**
   * Builds an `InitializeConfig` instruction.
   */
  async initializeConfig(params: {
    admin: Keypair;
    walletFee: bigint | number;
    actionFee: bigint | number;
    numShards: number;
  }): Promise<TransactionInstruction> {
    const configPda = findConfigPda(this.programId)[0];
    return this.builder.initializeConfig({
      admin: params.admin.publicKey,
      config: configPda,
      walletFee: BigInt(params.walletFee),
      actionFee: BigInt(params.actionFee),
      numShards: params.numShards,
    });
  }

  /**
   * Builds an `UpdateConfig` instruction.
   */
  async updateConfig(params: {
    admin: Keypair;
    walletFee?: bigint | number;
    actionFee?: bigint | number;
    numShards?: number;
    newAdmin?: PublicKey;
    configPda?: PublicKey;
  }): Promise<TransactionInstruction> {
    const configPda = params.configPda ?? findConfigPda(this.programId)[0];
    return this.builder.updateConfig({
      admin: params.admin.publicKey,
      config: configPda,
      walletFee: params.walletFee,
      actionFee: params.actionFee,
      numShards: params.numShards,
      newAdmin: params.newAdmin,
    });
  }

  /**
   * Builds an `InitTreasuryShard` instruction.
   */
  async initTreasuryShard(params: {
    payer: Keypair;
    shardId: number;
  }): Promise<TransactionInstruction> {
    const configPda = findConfigPda(this.programId)[0];
    const [treasuryShard] = findTreasuryShardPda(
      params.shardId,
      this.programId,
    );
    return this.builder.initTreasuryShard({
      payer: params.payer.publicKey,
      config: configPda,
      treasuryShard,
      shardId: params.shardId,
    });
  }

  /**
   * Builds a `SweepTreasury` instruction.
   */
  async sweepTreasury(params: {
    admin: Keypair;
    shardId: number;
    destination: PublicKey;
  }): Promise<TransactionInstruction> {
    const configPda = findConfigPda(this.programId)[0];
    const [treasuryShard] = findTreasuryShardPda(
      params.shardId,
      this.programId,
    );
    return this.builder.sweepTreasury({
      admin: params.admin.publicKey,
      config: configPda,
      treasuryShard,
      destination: params.destination,
      shardId: params.shardId,
    });
  }

  // ─── Layer 2: Transaction builders ───────────────────────────────────────
  // Return a `Transaction` object with `feePayer` set. Signing and sending
  // is always the caller's responsibility.

  async createWalletTxn(
    params: Parameters<typeof this.createWallet>[0],
  ): Promise<{
    transaction: Transaction;
    walletPda: PublicKey;
    authorityPda: PublicKey;
    userSeed: Uint8Array;
  }> {
    const { ix, walletPda, authorityPda, userSeed } = await this.createWallet(
      params,
    );
    const transaction = new Transaction().add(ix);
    transaction.feePayer = params.payer.publicKey;
    return { transaction, walletPda, authorityPda, userSeed };
  }

  async addAuthorityTxn(
    params: Parameters<typeof this.addAuthority>[0],
  ): Promise<{
    transaction: Transaction;
    newAuthority: PublicKey;
  }> {
    const { ix, newAuthority } = await this.addAuthority(params);
    const transaction = new Transaction().add(ix);
    transaction.feePayer = params.payer.publicKey;
    return { transaction, newAuthority };
  }

  async createSessionTxn(
    params: Parameters<typeof this.createSession>[0],
  ): Promise<{
    transaction: Transaction;
    sessionPda: PublicKey;
    sessionKeypair: Keypair | null;
  }> {
    const { ix, sessionPda, sessionKeypair } = await this.createSession(params);
    const transaction = new Transaction().add(ix);
    transaction.feePayer = params.payer.publicKey;
    return { transaction, sessionPda, sessionKeypair };
  }

  async executeTxn(
    params: Parameters<typeof this.execute>[0],
  ): Promise<Transaction> {
    const ix = await this.execute(params);
    const transaction = new Transaction().add(ix);
    transaction.feePayer = params.payer.publicKey;
    return transaction;
  }

  // ─── Discovery helpers ────────────────────────────────────────────────────

  /**
   * Finds all Wallet PDAs associated with a given Ed25519 public key.
   */
  static async findWalletsByEd25519Pubkey(
    connection: Connection,
    ed25519Pubkey: PublicKey,
    programId: PublicKey = PROGRAM_ID,
  ): Promise<PublicKey[]> {
    const accounts = await connection.getProgramAccounts(programId, {
      filters: [{ dataSize: AUTHORITY_ACCOUNT_ED25519_SIZE }], // Ed25519 authority size
    });

    const results: PublicKey[] = [];
    for (const a of accounts) {
      const data = a.account.data;
      if (data[0] === 2 && data[1] === 0) {
        // disc=2 (Authority), type=0 (Ed25519)
        const storedPubkey = data.subarray(48, 80);
        if (Buffer.compare(storedPubkey, ed25519Pubkey.toBuffer()) === 0) {
          results.push(new PublicKey(data.subarray(16, 48)));
        }
      }
    }
    return results;
  }

  /**
   * Finds all Wallet PDAs associated with a Secp256r1 credential hash.
   */
  static async findWalletsByCredentialHash(
    connection: Connection,
    credentialHash: Uint8Array,
    programId: PublicKey = PROGRAM_ID,
  ): Promise<PublicKey[]> {
    const accounts = await connection.getProgramAccounts(programId, {
      filters: [{ dataSize: AUTHORITY_ACCOUNT_SECP256R1_SIZE }], // Secp256r1 authority size
    });

    const results: PublicKey[] = [];
    for (const a of accounts) {
      const data = a.account.data;
      if (data[0] === 2 && data[1] === 1) {
        // disc=2 (Authority), type=1 (Secp256r1)
        const storedHash = data.subarray(48, 80);
        if (Buffer.compare(storedHash, Buffer.from(credentialHash)) === 0) {
          results.push(new PublicKey(data.subarray(16, 48)));
        }
      }
    }
    return results;
  }

  /**
   * Finds all Authority PDA records associated with a given Secp256r1 credential hash.
   *
   * Unlike `findWalletByCredentialHash` which only returns Wallet PDAs, this method
   * returns rich authority metadata — useful for credential-based wallet recovery flows.
   *
   * Account memory layout (Authority PDA):
   * - byte 0: discriminator (2 = Authority)
   * - byte 1: authority_type (0 = Ed25519, 1 = Secp256r1)
   * - byte 2: role (0 = Owner, 1 = Admin, 2 = Spender)
   * - bytes 16–48: wallet PDA
   * - bytes 48–80: credential_id_hash (Secp256r1) or pubkey (Ed25519)
   */
  static async findAllAuthoritiesByCredentialHash(
    connection: Connection,
    credentialHash: Uint8Array,
    programId: PublicKey = PROGRAM_ID,
  ): Promise<
    Array<{
      authority: PublicKey;
      wallet: PublicKey;
      role: number;
      authorityType: number;
    }>
  > {
    const accounts = await connection.getProgramAccounts(programId, {
      filters: [
        { dataSize: AUTHORITY_ACCOUNT_SECP256R1_SIZE }, // Secp256r1 authority size
        {
          memcmp: {
            offset: 0,
            bytes: bs58.encode(Buffer.from([DISCRIMINATOR_AUTHORITY])),
          },
        }, // disc = Authority
        {
          memcmp: {
            offset: 48,
            bytes: bs58.encode(Buffer.from(credentialHash)),
          },
        }, // credentialHash
      ],
    });

    return accounts.map((a) => {
      const data = a.account.data;
      return {
        authority: a.pubkey,
        wallet: new PublicKey(data.subarray(16, 48)),
        role: data[2],
        authorityType: data[1],
      };
    });
  }
}
