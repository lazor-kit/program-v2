/**
 * Hand-written instruction builders that produce the exact raw binary format
 * the LazorKit program expects. Solita-generated builders use beet which adds
 * length prefixes to `bytes` fields, causing a mismatch.
 */
import {
  PublicKey,
  TransactionInstruction,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from '@solana/web3.js';
import { PROGRAM_ID } from '../generated';

// ─── Discriminators ──────────────────────────────────────────────────
export const DISC_CREATE_WALLET = 0;
export const DISC_ADD_AUTHORITY = 1;
export const DISC_REMOVE_AUTHORITY = 2;
export const DISC_TRANSFER_OWNERSHIP = 3;
export const DISC_EXECUTE = 4;
export const DISC_CREATE_SESSION = 5;
export const DISC_AUTHORIZE = 6;
export const DISC_EXECUTE_DEFERRED = 7;
export const DISC_RECLAIM_DEFERRED = 8;

// ─── Authority types ─────────────────────────────────────────────────
export const AUTH_TYPE_ED25519 = 0;
export const AUTH_TYPE_SECP256R1 = 1;

// ─── Roles ───────────────────────────────────────────────────────────
export const ROLE_OWNER = 0;
export const ROLE_ADMIN = 1;
export const ROLE_SPENDER = 2;

// ─── CreateWallet ────────────────────────────────────────────────────
/**
 * Instruction data layout (after discriminator):
 *   [user_seed(32)][auth_type(1)][auth_bump(1)][padding(6)]
 *   Ed25519:   [pubkey(32)]
 *   Secp256r1: [credential_id_hash(32)][pubkey(33)][rpIdLen(1)][rpId(N)]
 */
export function createCreateWalletIx(params: {
  payer: PublicKey;
  walletPda: PublicKey;
  vaultPda: PublicKey;
  authorityPda: PublicKey;
  userSeed: Uint8Array;
  authType: number;
  authBump: number;
  /** Ed25519: 32-byte pubkey. Secp256r1: 32-byte credential_id_hash */
  credentialOrPubkey: Uint8Array;
  /** Secp256r1 only: 33-byte compressed pubkey */
  secp256r1Pubkey?: Uint8Array;
  /** Secp256r1 only: RP ID string (stored on-chain for per-tx savings) */
  rpId?: string;
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;
  const parts: Uint8Array[] = [
    new Uint8Array([DISC_CREATE_WALLET]),
    params.userSeed,
    new Uint8Array([params.authType, params.authBump]),
    new Uint8Array(6), // padding
    params.credentialOrPubkey,
  ];
  if (params.authType === AUTH_TYPE_SECP256R1 && params.secp256r1Pubkey) {
    parts.push(params.secp256r1Pubkey);
    if (params.rpId) {
      const rpIdBytes = Buffer.from(params.rpId, 'utf-8');
      parts.push(new Uint8Array([rpIdBytes.length]));
      parts.push(new Uint8Array(rpIdBytes));
    }
  }

  return new TransactionInstruction({
    programId: pid,
    keys: [
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: params.walletPda, isSigner: false, isWritable: true },
      { pubkey: params.vaultPda, isSigner: false, isWritable: true },
      { pubkey: params.authorityPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    ],
    data: Buffer.from(concatBytes(parts)),
  });
}

// ─── AddAuthority ────────────────────────────────────────────────────
/**
 * Instruction data layout (after discriminator):
 *   [auth_type(1)][new_role(1)][padding(6)]
 *   Ed25519:   [pubkey(32)]
 *   Secp256r1: [credential_id_hash(32)][pubkey(33)][rpIdLen(1)][rpId(N)] + [auth_payload(...)]
 */
export function createAddAuthorityIx(params: {
  payer: PublicKey;
  walletPda: PublicKey;
  adminAuthorityPda: PublicKey;
  newAuthorityPda: PublicKey;
  newType: number;
  newRole: number;
  /** Ed25519: 32-byte pubkey. Secp256r1: 32-byte credential_id_hash */
  credentialOrPubkey: Uint8Array;
  /** Secp256r1 only: 33-byte compressed pubkey */
  secp256r1Pubkey?: Uint8Array;
  /** Secp256r1 only: RP ID string for the new authority */
  rpId?: string;
  /** Auth payload for Secp256r1 admin authentication */
  authPayload?: Uint8Array;
  /** For Ed25519 admin: the signer pubkey */
  authorizerSigner?: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;
  const parts: Uint8Array[] = [
    new Uint8Array([DISC_ADD_AUTHORITY]),
    new Uint8Array([params.newType, params.newRole]),
    new Uint8Array(6), // padding
    params.credentialOrPubkey,
  ];
  if (params.newType === AUTH_TYPE_SECP256R1 && params.secp256r1Pubkey) {
    parts.push(params.secp256r1Pubkey);
    if (params.rpId) {
      const rpIdBytes = Buffer.from(params.rpId, 'utf-8');
      parts.push(new Uint8Array([rpIdBytes.length]));
      parts.push(new Uint8Array(rpIdBytes));
    }
  }
  if (params.authPayload) {
    parts.push(params.authPayload);
  }

  const keys = [
    { pubkey: params.payer, isSigner: true, isWritable: false },
    { pubkey: params.walletPda, isSigner: false, isWritable: false },
    { pubkey: params.adminAuthorityPda, isSigner: false, isWritable: true },
    { pubkey: params.newAuthorityPda, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
  ];

  // Secp256r1 auth needs sysvar instructions; Ed25519 needs the signer
  if (params.authorizerSigner) {
    keys.push({ pubkey: params.authorizerSigner, isSigner: true, isWritable: false });
  } else if (params.authPayload) {
    keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });
  }

  return new TransactionInstruction({
    programId: pid,
    keys,
    data: Buffer.from(concatBytes(parts)),
  });
}

// ─── RemoveAuthority ─────────────────────────────────────────────────
/**
 * Instruction data layout (after discriminator):
 *   Secp256r1: [auth_payload(...)]
 *   Ed25519:   empty (auth is via signer)
 */
export function createRemoveAuthorityIx(params: {
  payer: PublicKey;
  walletPda: PublicKey;
  adminAuthorityPda: PublicKey;
  targetAuthorityPda: PublicKey;
  refundDestination: PublicKey;
  authPayload?: Uint8Array;
  authorizerSigner?: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;
  const parts: Uint8Array[] = [new Uint8Array([DISC_REMOVE_AUTHORITY])];
  if (params.authPayload) {
    parts.push(params.authPayload);
  }

  const keys = [
    { pubkey: params.payer, isSigner: true, isWritable: false },
    { pubkey: params.walletPda, isSigner: false, isWritable: false },
    { pubkey: params.adminAuthorityPda, isSigner: false, isWritable: true },
    { pubkey: params.targetAuthorityPda, isSigner: false, isWritable: true },
    { pubkey: params.refundDestination, isSigner: false, isWritable: true },
  ];

  if (params.authorizerSigner) {
    keys.push({ pubkey: params.authorizerSigner, isSigner: true, isWritable: false });
  } else if (params.authPayload) {
    keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });
  }

  return new TransactionInstruction({
    programId: pid,
    keys,
    data: Buffer.from(concatBytes(parts)),
  });
}

// ─── TransferOwnership ──────────────────────────────────────────────
/**
 * Instruction data layout (after discriminator):
 *   [auth_type(1)]
 *   Ed25519:   [pubkey(32)]
 *   Secp256r1: [credential_id_hash(32)][pubkey(33)][rpIdLen(1)][rpId(N)] + [auth_payload(...)]
 */
export function createTransferOwnershipIx(params: {
  payer: PublicKey;
  walletPda: PublicKey;
  currentOwnerAuthorityPda: PublicKey;
  newOwnerAuthorityPda: PublicKey;
  newType: number;
  /** Ed25519: 32-byte pubkey. Secp256r1: 32-byte credential_id_hash */
  credentialOrPubkey: Uint8Array;
  /** Secp256r1 only: 33-byte compressed pubkey */
  secp256r1Pubkey?: Uint8Array;
  /** Secp256r1 only: RP ID string for the new owner */
  rpId?: string;
  authPayload?: Uint8Array;
  authorizerSigner?: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;
  const parts: Uint8Array[] = [
    new Uint8Array([DISC_TRANSFER_OWNERSHIP]),
    new Uint8Array([params.newType]),
    params.credentialOrPubkey,
  ];
  if (params.newType === AUTH_TYPE_SECP256R1 && params.secp256r1Pubkey) {
    parts.push(params.secp256r1Pubkey);
    if (params.rpId) {
      const rpIdBytes = Buffer.from(params.rpId, 'utf-8');
      parts.push(new Uint8Array([rpIdBytes.length]));
      parts.push(new Uint8Array(rpIdBytes));
    }
  }
  if (params.authPayload) {
    parts.push(params.authPayload);
  }

  const keys = [
    { pubkey: params.payer, isSigner: true, isWritable: false },
    { pubkey: params.walletPda, isSigner: false, isWritable: false },
    { pubkey: params.currentOwnerAuthorityPda, isSigner: false, isWritable: true },
    { pubkey: params.newOwnerAuthorityPda, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
  ];

  if (params.authorizerSigner) {
    keys.push({ pubkey: params.authorizerSigner, isSigner: true, isWritable: false });
  } else if (params.authPayload) {
    keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });
  }

  return new TransactionInstruction({
    programId: pid,
    keys,
    data: Buffer.from(concatBytes(parts)),
  });
}

// ─── Execute ─────────────────────────────────────────────────────────
/**
 * Instruction data layout (after discriminator):
 *   [compact_instructions(variable)]
 *   Secp256r1: [auth_payload(variable)]
 */
export function createExecuteIx(params: {
  payer: PublicKey;
  walletPda: PublicKey;
  authorityPda: PublicKey;
  vaultPda: PublicKey;
  packedInstructions: Uint8Array;
  authPayload?: Uint8Array;
  /** For Ed25519 auth: the signer pubkey (placed at account index 4) */
  authorizerSigner?: PublicKey;
  /** Additional account metas for the inner CPI instructions */
  remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;
  const parts: Uint8Array[] = [
    new Uint8Array([DISC_EXECUTE]),
    params.packedInstructions,
  ];
  if (params.authPayload) {
    parts.push(params.authPayload);
  }

  const keys = [
    { pubkey: params.payer, isSigner: true, isWritable: false },
    { pubkey: params.walletPda, isSigner: false, isWritable: false },
    { pubkey: params.authorityPda, isSigner: false, isWritable: true },
    { pubkey: params.vaultPda, isSigner: false, isWritable: true },
  ];

  // Ed25519 needs the signer at index 4; Secp256r1 needs sysvar instructions
  if (params.authorizerSigner) {
    keys.push({ pubkey: params.authorizerSigner, isSigner: true, isWritable: false });
  } else if (params.authPayload) {
    keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });
  }

  // Remaining accounts for CPI targets
  if (params.remainingAccounts) {
    keys.push(...params.remainingAccounts);
  }

  return new TransactionInstruction({
    programId: pid,
    keys,
    data: Buffer.from(concatBytes(parts)),
  });
}

// ─── CreateSession ───────────────────────────────────────────────────
/**
 * Instruction data layout (after discriminator):
 *   [session_key(32)][expires_at(8)]
 *   Secp256r1: [auth_payload(variable)]
 */
export function createCreateSessionIx(params: {
  payer: PublicKey;
  walletPda: PublicKey;
  adminAuthorityPda: PublicKey;
  sessionPda: PublicKey;
  sessionKey: Uint8Array;
  expiresAt: bigint;
  authPayload?: Uint8Array;
  authorizerSigner?: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;
  const expiresAtBuf = Buffer.alloc(8);
  expiresAtBuf.writeBigInt64LE(params.expiresAt);

  const parts: Uint8Array[] = [
    new Uint8Array([DISC_CREATE_SESSION]),
    params.sessionKey,
    new Uint8Array(expiresAtBuf),
  ];
  if (params.authPayload) {
    parts.push(params.authPayload);
  }

  const keys = [
    { pubkey: params.payer, isSigner: true, isWritable: false },
    { pubkey: params.walletPda, isSigner: false, isWritable: false },
    { pubkey: params.adminAuthorityPda, isSigner: false, isWritable: true },
    { pubkey: params.sessionPda, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
  ];

  if (params.authorizerSigner) {
    keys.push({ pubkey: params.authorizerSigner, isSigner: true, isWritable: false });
  } else if (params.authPayload) {
    keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false });
  }

  return new TransactionInstruction({
    programId: pid,
    keys,
    data: Buffer.from(concatBytes(parts)),
  });
}

// ─── Authorize (Deferred Execution tx1) ─────────────────────────────
/**
 * Instruction data layout (after discriminator):
 *   [instructions_hash(32)][accounts_hash(32)][expiry_offset(2)][auth_payload(variable)]
 */
export function createAuthorizeIx(params: {
  payer: PublicKey;
  walletPda: PublicKey;
  authorityPda: PublicKey;
  deferredExecPda: PublicKey;
  instructionsHash: Uint8Array;
  accountsHash: Uint8Array;
  expiryOffset: number;
  authPayload: Uint8Array;
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;
  const expiryBuf = Buffer.alloc(2);
  expiryBuf.writeUInt16LE(params.expiryOffset);

  const parts: Uint8Array[] = [
    new Uint8Array([DISC_AUTHORIZE]),
    params.instructionsHash,
    params.accountsHash,
    new Uint8Array(expiryBuf),
    params.authPayload,
  ];

  return new TransactionInstruction({
    programId: pid,
    keys: [
      { pubkey: params.payer, isSigner: true, isWritable: true },
      { pubkey: params.walletPda, isSigner: false, isWritable: false },
      { pubkey: params.authorityPda, isSigner: false, isWritable: true },
      { pubkey: params.deferredExecPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
    ],
    data: Buffer.from(concatBytes(parts)),
  });
}

// ─── ExecuteDeferred (Deferred Execution tx2) ───────────────────────
/**
 * Instruction data layout (after discriminator):
 *   [compact_instructions(variable)]
 */
export function createExecuteDeferredIx(params: {
  payer: PublicKey;
  walletPda: PublicKey;
  vaultPda: PublicKey;
  deferredExecPda: PublicKey;
  refundDestination: PublicKey;
  packedInstructions: Uint8Array;
  /** Additional account metas for the inner CPI instructions */
  remainingAccounts?: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;
  const parts: Uint8Array[] = [
    new Uint8Array([DISC_EXECUTE_DEFERRED]),
    params.packedInstructions,
  ];

  const keys = [
    { pubkey: params.payer, isSigner: true, isWritable: true },
    { pubkey: params.walletPda, isSigner: false, isWritable: false },
    { pubkey: params.vaultPda, isSigner: false, isWritable: true },
    { pubkey: params.deferredExecPda, isSigner: false, isWritable: true },
    { pubkey: params.refundDestination, isSigner: false, isWritable: true },
  ];

  if (params.remainingAccounts) {
    keys.push(...params.remainingAccounts);
  }

  return new TransactionInstruction({
    programId: pid,
    keys,
    data: Buffer.from(concatBytes(parts)),
  });
}

// ─── ReclaimDeferred ────────────────────────────────────────────────
/**
 * Closes an expired DeferredExec account and refunds rent.
 * Instruction data: discriminator only (no payload).
 */
export function createReclaimDeferredIx(params: {
  payer: PublicKey;
  deferredExecPda: PublicKey;
  refundDestination: PublicKey;
  programId?: PublicKey;
}): TransactionInstruction {
  const pid = params.programId ?? PROGRAM_ID;

  return new TransactionInstruction({
    programId: pid,
    keys: [
      { pubkey: params.payer, isSigner: true, isWritable: false },
      { pubkey: params.deferredExecPda, isSigner: false, isWritable: true },
      { pubkey: params.refundDestination, isSigner: false, isWritable: true },
    ],
    data: Buffer.from([DISC_RECLAIM_DEFERRED]),
  });
}

// ─── Helper ──────────────────────────────────────────────────────────
function concatBytes(arrays: Uint8Array[]): Uint8Array {
  const totalLen = arrays.reduce((s, a) => s + a.length, 0);
  const out = new Uint8Array(totalLen);
  let offset = 0;
  for (const a of arrays) {
    out.set(a, offset);
    offset += a.length;
  }
  return out;
}
