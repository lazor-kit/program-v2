import {
  Connection,
  PublicKey,
  TransactionInstruction,
} from '@solana/web3.js';
import { PROGRAM_ID } from '../generated';
import { AuthorityAccount } from '../generated/accounts';
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda, findDeferredExecPda } from './pdas';
import {
  readAuthorityCounter,
  buildAuthPayload,
  buildSecp256r1Challenge,
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
  }): { ix: TransactionInstruction; newAuthorityPda: PublicKey } {
    const credHash = params.newType === AUTH_TYPE_SECP256R1
      ? params.newCredentialOrPubkey
      : params.newCredentialOrPubkey; // Ed25519 uses pubkey bytes as hash
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
      authorizerSigner: params.adminSigner,
      programId: this.programId,
    });

    return { ix, newAuthorityPda };
  }

  // ─── AddAuthority (Secp256r1 admin) ──────────────────────────────
  async addAuthoritySecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    adminAuthorityPda: PublicKey;
    adminSigner: Secp256r1Signer;
    slot: bigint;
    sysvarIxIndex: number;
    newType: number;
    newRole: number;
    newCredentialOrPubkey: Uint8Array;
    newSecp256r1Pubkey?: Uint8Array;
    newRpId?: string;
  }): Promise<{ ix: TransactionInstruction; newAuthorityPda: PublicKey; precompileIx: TransactionInstruction }> {
    const credHash = params.newCredentialOrPubkey;
    const [newAuthorityPda] = this.findAuthority(params.walletPda, credHash);

    const counter = (await this.readCounter(params.adminAuthorityPda)) + 1;

    // Build the data payload (what's signed)
    const dataPayload = buildDataPayloadForAdd(
      params.newType, params.newRole,
      params.newCredentialOrPubkey, params.newSecp256r1Pubkey,
    );

    // Build challenge hash
    const challenge = buildSecp256r1Challenge({
      discriminator: new Uint8Array([DISC_ADD_AUTHORITY]),
      authPayload: new Uint8Array(0), // placeholder — filled after sign
      signedPayload: dataPayload,
      slot: params.slot,
      payer: params.payer,
      counter,
      programId: this.programId,
    });

    // Sign
    const { signature, authenticatorData } = await params.adminSigner.sign(challenge);

    // Build auth payload with real authenticator data
    const authPayload = buildAuthPayload({
      slot: params.slot,
      counter,
      sysvarIxIndex: params.sysvarIxIndex,
      typeAndFlags: 0,
      authenticatorData,
    });

    // Build the Secp256r1 precompile instruction
    const precompileIx = buildSecp256r1PrecompileIx(
      params.adminSigner.publicKeyBytes,
      challenge,
      signature,
    );

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
    refundDestination: PublicKey;
  }): TransactionInstruction {
    return createRemoveAuthorityIx({
      payer: params.payer,
      walletPda: params.walletPda,
      adminAuthorityPda: params.adminAuthorityPda,
      targetAuthorityPda: params.targetAuthorityPda,
      refundDestination: params.refundDestination,
      authorizerSigner: params.adminSigner,
      programId: this.programId,
    });
  }

  // ─── Execute (Ed25519) ───────────────────────────────────────────
  executeEd25519(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    authorityPda: PublicKey;
    vaultPda: PublicKey;
    compactInstructions: CompactInstruction[];
    remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  }): TransactionInstruction {
    const packed = packCompactInstructions(params.compactInstructions);
    return createExecuteIx({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityPda: params.authorityPda,
      vaultPda: params.vaultPda,
      packedInstructions: packed,
      remainingAccounts: params.remainingAccounts,
      programId: this.programId,
    });
  }

  // ─── Execute (Secp256r1) ─────────────────────────────────────────
  async executeSecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    authorityPda: PublicKey;
    vaultPda: PublicKey;
    signer: Secp256r1Signer;
    slot: bigint;
    sysvarIxIndex: number;
    compactInstructions: CompactInstruction[];
    remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  }): Promise<{ ix: TransactionInstruction; precompileIx: TransactionInstruction }> {
    const counter = (await this.readCounter(params.authorityPda)) + 1;
    const packed = packCompactInstructions(params.compactInstructions);

    // Build challenge
    const challenge = buildSecp256r1Challenge({
      discriminator: new Uint8Array([DISC_EXECUTE]),
      authPayload: new Uint8Array(0),
      signedPayload: packed,
      slot: params.slot,
      payer: params.payer,
      counter,
      programId: this.programId,
    });

    const { signature, authenticatorData } = await params.signer.sign(challenge);

    const authPayload = buildAuthPayload({
      slot: params.slot,
      counter,
      sysvarIxIndex: params.sysvarIxIndex,
      typeAndFlags: 0,
      authenticatorData,
    });

    const precompileIx = buildSecp256r1PrecompileIx(
      params.signer.publicKeyBytes,
      challenge,
      signature,
    );

    const ix = createExecuteIx({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityPda: params.authorityPda,
      vaultPda: params.vaultPda,
      packedInstructions: packed,
      authPayload,
      remainingAccounts: params.remainingAccounts,
      programId: this.programId,
    });

    return { ix, precompileIx };
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

  // ─── Authorize (Deferred Execution tx1, Secp256r1) ───────────────
  async authorizeSecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    authorityPda: PublicKey;
    signer: Secp256r1Signer;
    slot: bigint;
    sysvarIxIndex: number;
    compactInstructions: CompactInstruction[];
    /** Account metas for the tx2 (ExecuteDeferred) layout */
    tx2AccountMetas: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
    /** Expiry offset in slots (default 300 = ~2 minutes) */
    expiryOffset?: number;
  }): Promise<{
    authorizeIx: TransactionInstruction;
    precompileIx: TransactionInstruction;
    deferredExecPda: PublicKey;
    counter: number;
  }> {
    const counter = (await this.readCounter(params.authorityPda)) + 1;
    const expiryOffset = params.expiryOffset ?? 300;

    // Compute instruction hash and accounts hash for tx2
    const instructionsHash = computeInstructionsHash(params.compactInstructions);
    const accountsHash = computeAccountsHash(params.tx2AccountMetas, params.compactInstructions);

    // signed_payload = instructions_hash || accounts_hash
    const signedPayload = new Uint8Array(64);
    signedPayload.set(instructionsHash, 0);
    signedPayload.set(accountsHash, 32);

    // Build challenge hash
    const challenge = buildSecp256r1Challenge({
      discriminator: new Uint8Array([DISC_AUTHORIZE]),
      authPayload: new Uint8Array(0),
      signedPayload,
      slot: params.slot,
      payer: params.payer,
      counter,
      programId: this.programId,
    });

    // Sign
    const { signature, authenticatorData } = await params.signer.sign(challenge);

    // Build auth payload
    const authPayload = buildAuthPayload({
      slot: params.slot,
      counter,
      sysvarIxIndex: params.sysvarIxIndex,
      typeAndFlags: 0,
      authenticatorData,
    });

    // Build precompile instruction
    const precompileIx = buildSecp256r1PrecompileIx(
      params.signer.publicKeyBytes,
      challenge,
      signature,
    );

    // Derive DeferredExec PDA using the counter value
    const [deferredExecPda] = this.findDeferredExec(
      params.walletPda,
      params.authorityPda,
      counter,
    );

    const authorizeIx = createAuthorizeIx({
      payer: params.payer,
      walletPda: params.walletPda,
      authorityPda: params.authorityPda,
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
  executeDeferredSecp256r1(params: {
    payer: PublicKey;
    walletPda: PublicKey;
    vaultPda: PublicKey;
    deferredExecPda: PublicKey;
    refundDestination: PublicKey;
    compactInstructions: CompactInstruction[];
    remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  }): TransactionInstruction {
    const packed = packCompactInstructions(params.compactInstructions);
    return createExecuteDeferredIx({
      payer: params.payer,
      walletPda: params.walletPda,
      vaultPda: params.vaultPda,
      deferredExecPda: params.deferredExecPda,
      refundDestination: params.refundDestination,
      packedInstructions: packed,
      remainingAccounts: params.remainingAccounts,
      programId: this.programId,
    });
  }

  // ─── ReclaimDeferred ────────────────────────────────────────────
  reclaimDeferred(params: {
    payer: PublicKey;
    deferredExecPda: PublicKey;
    refundDestination: PublicKey;
  }): TransactionInstruction {
    return createReclaimDeferredIx({
      payer: params.payer,
      deferredExecPda: params.deferredExecPda,
      refundDestination: params.refundDestination,
      programId: this.programId,
    });
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
      authorizerSigner: params.ownerSigner,
      programId: this.programId,
    });

    return { ix, newOwnerAuthorityPda };
  }
}

// ─── Internal helpers ────────────────────────────────────────────────

function buildDataPayloadForAdd(
  newType: number,
  newRole: number,
  credentialOrPubkey: Uint8Array,
  secp256r1Pubkey?: Uint8Array,
): Uint8Array {
  const parts: Uint8Array[] = [
    new Uint8Array([newType, newRole]),
    new Uint8Array(6), // padding
    credentialOrPubkey,
  ];
  if (newType === AUTH_TYPE_SECP256R1 && secp256r1Pubkey) {
    parts.push(secp256r1Pubkey);
  }
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
 * Layout:
 *   [num_signatures(1)][padding(1)]
 *   [sig_offset(2)][sig_ix_index(2)]
 *   [pubkey_offset(2)][pubkey_ix_index(2)]
 *   [msg_offset(2)][msg_size(2)][msg_ix_index(2)]
 *   [signature(64)][pubkey(33)][message(N)]
 */
function buildSecp256r1PrecompileIx(
  publicKey: Uint8Array,
  message: Uint8Array,
  signature: Uint8Array,
): TransactionInstruction {
  const SECP256R1_PROGRAM_ID = new PublicKey('Secp256r1SigVerify1111111111111111111111111');

  const headerSize = 2 + 2 + 2 + 2 + 2 + 2 + 2 + 2; // 16 bytes
  const sigOffset = headerSize;
  const pubkeyOffset = sigOffset + 64;
  const msgOffset = pubkeyOffset + 33;

  const data = Buffer.alloc(headerSize + 64 + 33 + message.length);
  let off = 0;

  // Header
  data.writeUInt8(1, off); off += 1;              // num_signatures
  data.writeUInt8(0, off); off += 1;              // padding
  data.writeUInt16LE(sigOffset, off); off += 2;   // sig_offset
  data.writeUInt16LE(0xFFFF, off); off += 2;      // sig_ix_index (same tx = 0xFFFF)
  data.writeUInt16LE(pubkeyOffset, off); off += 2; // pubkey_offset
  data.writeUInt16LE(0xFFFF, off); off += 2;      // pubkey_ix_index
  data.writeUInt16LE(msgOffset, off); off += 2;   // msg_offset
  data.writeUInt16LE(message.length, off); off += 2; // msg_size
  data.writeUInt16LE(0xFFFF, off); off += 2;      // msg_ix_index

  // Payload
  Buffer.from(signature).copy(data, off); off += 64;
  Buffer.from(publicKey).copy(data, off); off += 33;
  Buffer.from(message).copy(data, off);

  return new TransactionInstruction({
    programId: SECP256R1_PROGRAM_ID,
    keys: [],
    data,
  });
}
