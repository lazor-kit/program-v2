/**
 * Secp256r1 utility helpers for LazorKit SDK.
 *
 * This module provides pure cryptographic building blocks for constructing
 * Secp256r1 precompile instructions and auth payloads used by LazorKit's
 * on-chain authentication flow.
 *
 * **These are NOT mocking utilities** — the `Secp256r1Signer` interface
 * is designed to be implemented by callers with the real signing mechanism
 * (hardware key, WebAuthn, etc.). For testing, see the test-specific
 * `generateMockSecp256r1Signer` helper in the test suite.
 */

import { PublicKey, TransactionInstruction, Connection } from '@solana/web3.js';
// Remove node:crypto import
async function sha256(data: Uint8Array): Promise<Uint8Array> {
  const hashBuffer = await crypto.subtle.digest(
    'SHA-256',
    data as unknown as BufferSource,
  );
  return new Uint8Array(hashBuffer);
}

// ─── Types ───────────────────────────────────────────────────────────────────

/**
 * Minimal interface a Secp256r1 signer must implement.
 * The SDK does not depend on any specific crypto library.
 */
export interface Secp256r1Signer {
  /** 33-byte compressed P-256 public key */
  publicKeyBytes: Uint8Array;
  /** 32-byte SHA-256 hash of the WebAuthn credential ID */
  credentialIdHash: Uint8Array;
  /** Sign a message, returning a 64-byte raw r‖s signature (low-S enforced) */
  sign(message: Uint8Array): Promise<Uint8Array>;
}

/** Sysvar public keys used by LazorKit's Secp256r1 auth flow */
export const SYSVAR_INSTRUCTIONS_PUBKEY = new PublicKey(
  'Sysvar1nstructions1111111111111111111111111',
);
export const SYSVAR_SLOT_HASHES_PUBKEY = new PublicKey(
  'SysvarS1otHashes111111111111111111111111111',
);

// ─── Sysvar helpers ───────────────────────────────────────────────────────────

/**
 * Appends the two sysvars required by LazorKit's Secp256r1 auth to an
 * instruction's account list.
 *
 * @returns The mutated instruction plus the indices of the two sysvars,
 *          which are needed when building the auth payload.
 */
export function appendSecp256r1Sysvars(ix: TransactionInstruction): {
  ix: TransactionInstruction;
  sysvarIxIndex: number;
  sysvarSlotIndex: number;
} {
  const sysvarIxIndex = ix.keys.length;
  const sysvarSlotIndex = ix.keys.length + 1;

  ix.keys.push(
    { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_SLOT_HASHES_PUBKEY, isSigner: false, isWritable: false },
  );

  return { ix, sysvarIxIndex, sysvarSlotIndex };
}

/**
 * Reads the current slot number from the `SlotHashes` sysvar on-chain.
 */
export async function readCurrentSlot(connection: Connection): Promise<bigint> {
  const accountInfo = await connection.getAccountInfo(
    SYSVAR_SLOT_HASHES_PUBKEY,
  );
  if (!accountInfo) throw new Error('SlotHashes sysvar not found');
  const data = accountInfo.data;
  return new DataView(
    data.buffer,
    data.byteOffset,
    data.byteLength,
  ).getBigUint64(8, true);
}

// ─── Payload builders ─────────────────────────────────────────────────────────

/**
 * Builds the `AuthPayload` that encodes the Secp256r1 liveness proof context.
 *
 * The payload layout is:
 * `[slot(8)] [sysvarIxIndex(1)] [sysvarSlotIndex(1)] [flags(1)] [rpIdLen(1)] [rpId(N)] [authenticatorData(M)]`
 *
 * @param sysvarIxIndex    Account index of SysvarInstructions in the instruction's account list
 * @param sysvarSlotIndex  Account index of SysvarSlotHashes in the instruction's account list
 * @param authenticatorData  37-byte WebAuthn authenticator data (rpIdHash + flags + counter)
 * @param slot             The current slot (from SlotHashes). Default: 0n
 * @param rpId             Relying party ID. Default: "example.com"
 */
export function buildAuthPayload(params: {
  sysvarIxIndex: number;
  sysvarSlotIndex: number;
  authenticatorData: Uint8Array;
  slot?: bigint;
  rpId?: string;
}): Uint8Array {
  const { sysvarIxIndex, sysvarSlotIndex, authenticatorData } = params;
  const slot = params.slot ?? 0n;
  const rpId = params.rpId ?? 'example.com';
  const rpIdBytes = new TextEncoder().encode(rpId);

  const payloadLen = 12 + rpIdBytes.length + authenticatorData.length;
  const payload = new Uint8Array(payloadLen);
  const view = new DataView(
    payload.buffer,
    payload.byteOffset,
    payload.byteLength,
  );

  view.setBigUint64(0, slot, true);
  payload[8] = sysvarIxIndex;
  payload[9] = sysvarSlotIndex;
  payload[10] = 0x10; // webauthn.get | https scheme flag
  payload[11] = rpIdBytes.length;
  payload.set(rpIdBytes, 12);
  payload.set(authenticatorData, 12 + rpIdBytes.length);

  return payload;
}

/**
 * Builds a standard 37-byte WebAuthn authenticator data structure.
 *
 * @param rpId  Relying party ID. Default: "example.com"
 * @param counter  Monotonically increasing counter to prevent replay attacks (big-endian u32). Default: 1
 */
export async function buildAuthenticatorData(
  rpId = 'example.com',
  counter = 1,
): Promise<Uint8Array> {
  const rpIdBytes = new TextEncoder().encode(rpId);
  const rpIdHash = await sha256(rpIdBytes);
  const data = new Uint8Array(37);
  data.set(rpIdHash, 0); // 32 bytes: rpIdHash
  data[32] = 0x01; // User Present flag
  // bytes 33-36: counter (big-endian u32, must be > 0 to prevent replay)
  const counterView = new DataView(data.buffer, 33, 4);
  counterView.setUint32(0, counter, false); // false = big-endian
  return data;
}

/**
 * Computes the raw 69-byte message that gets signed by the Secp256r1 key.
 * This consists of the 37-byte authenticator data and the 32-byte SHA-256 hash of the clientDataJSON.
 *
 * The contract verifies this exact message construction on-chain.
 */
export async function buildSecp256r1Message(params: {
  /** Instruction discriminator byte (e.g. 0=CreateWallet, 1=AddAuthority, …) */
  discriminator: number;
  authPayload: Uint8Array;
  /** Instruction-specific signed data (varies per instruction) */
  signedPayload: Uint8Array;
  payer: PublicKey;
  programId: PublicKey;
  slot: bigint;
  /** Origin of the website requesting the signature (e.g. "https://my-dapp.com"). Defaults to "https://example.com" */
  origin?: string;
  /** Relying party ID used to build authenticatorData. Must match authPayload rpId. */
  rpId?: string;
  /** Counter value for authenticatorData. Must match counter used in authPayload. */
  counter?: number;
}): Promise<Uint8Array> {
  const {
    discriminator,
    authPayload,
    signedPayload,
    payer,
    programId,
    slot,
    origin,
    rpId,
    counter,
  } = params;

  const slotBytes = new Uint8Array(8);
  new DataView(slotBytes.buffer).setBigUint64(0, slot, true);

  // Concatenate all parts for challenge hashing
  const totalLen = 1 + authPayload.length + signedPayload.length + 8 + 32 + 32;
  const combined = new Uint8Array(totalLen);
  let offset = 0;

  combined[0] = discriminator;
  offset += 1;
  combined.set(authPayload, offset);
  offset += authPayload.length;
  combined.set(signedPayload, offset);
  offset += signedPayload.length;
  combined.set(slotBytes, offset);
  offset += 8;
  combined.set(payer.toBytes(), offset);
  offset += 32;
  combined.set(programId.toBytes(), offset);

  const challengeHash = await sha256(combined);

  // Encode challenge as base64url (no padding)
  const challengeB64 = Buffer.from(challengeHash)
    .toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=/g, '');

  const clientDataJson = JSON.stringify({
    type: 'webauthn.get',
    challenge: challengeB64,
    origin: origin ?? 'https://example.com',
    crossOrigin: false,
  });

  const authenticatorData = await buildAuthenticatorData(rpId ?? 'example.com', counter ?? 1);
  const clientDataHash = await sha256(new TextEncoder().encode(clientDataJson));

  const message = new Uint8Array(
    authenticatorData.length + clientDataHash.length,
  );
  message.set(authenticatorData, 0);
  message.set(clientDataHash, authenticatorData.length);
  return message;
}

// ─── Precompile instruction builder ──────────────────────────────────────────

export const SECP256R1_PROGRAM_ID = new PublicKey(
  'Secp256r1SigVerify1111111111111111111111111',
);

/**
 * Builds a Secp256r1 precompile instruction that verifies one signature.
 *
 * The message is first signed via `signer.sign(message)` (caller-provided),
 * then the full precompile instruction is constructed with proper offsets.
 *
 * @param signer   Implements `Secp256r1Signer` — provides sign() and key bytes
 * @param message  The raw 69-byte message: `authenticatorData ‖ sha256(clientDataJSON)`
 */
export async function buildSecp256r1PrecompileIx(
  signer: Secp256r1Signer,
  message: Uint8Array,
): Promise<TransactionInstruction> {
  const signature = await signer.sign(message);

  const OFFSETS_START = 2;
  const OFFSETS_SIZE = 14;
  const DATA_START = OFFSETS_START + OFFSETS_SIZE; // 16

  const signatureOffset = DATA_START;
  const pubkeyOffset = signatureOffset + 64;
  const msgOffset = pubkeyOffset + 33 + 1; // +1 padding

  const totalSize = msgOffset + message.length;
  const data = new Uint8Array(totalSize);

  data[0] = 1; // number of signatures
  data[1] = 0; // padding

  const view = new DataView(
    data.buffer,
    data.byteOffset + OFFSETS_START,
    OFFSETS_SIZE,
  );
  view.setUint16(0, signatureOffset, true);
  view.setUint16(2, 0xffff, true); // instruction index (0xffff = current)
  view.setUint16(4, pubkeyOffset, true);
  view.setUint16(6, 0xffff, true);
  view.setUint16(8, msgOffset, true);
  view.setUint16(10, message.length, true);
  view.setUint16(12, 0xffff, true);

  data.set(signature, signatureOffset);
  data.set(signer.publicKeyBytes, pubkeyOffset);
  data.set(message, msgOffset);

  return new TransactionInstruction({
    programId: SECP256R1_PROGRAM_ID,
    keys: [],
    data: Buffer.from(data),
  });
}
