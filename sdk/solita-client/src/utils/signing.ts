import { PublicKey, TransactionInstruction } from '@solana/web3.js';
import {
  buildAuthPayload,
  buildSecp256r1Challenge,
  generateAuthenticatorData,
  type Secp256r1Signer,
} from './secp256r1';
import { AUTH_TYPE_SECP256R1 } from './instructions';

// ─── Secp256r1 signing flow ─────────────────────────────────────────

/**
 * Full Secp256r1 signing: build auth payload, compute challenge, sign, build precompile ix.
 */
export async function signWithSecp256r1(params: {
  signer: Secp256r1Signer;
  discriminator: Uint8Array;
  signedPayload: Uint8Array;
  sysvarIxIndex: number;
  slot: bigint;
  counter: number;
  payer: PublicKey;
  programId: PublicKey;
}): Promise<{
  authPayload: Uint8Array;
  precompileIx: TransactionInstruction;
}> {
  const authenticatorData = generateAuthenticatorData(params.signer.rpId);

  const authPayload = buildAuthPayload({
    slot: params.slot,
    counter: params.counter,
    sysvarIxIndex: params.sysvarIxIndex,
    typeAndFlags: 0x10, // webauthn.get + https
    authenticatorData,
  });

  const challenge = buildSecp256r1Challenge({
    discriminator: params.discriminator,
    authPayload,
    signedPayload: params.signedPayload,
    slot: params.slot,
    payer: params.payer,
    counter: params.counter,
    programId: params.programId,
  });

  const { signature, authenticatorData: signerAuthData, clientDataJsonHash } =
    await params.signer.sign(challenge);

  const precompileMessage = concatParts([signerAuthData, clientDataJsonHash]);
  const precompileIx = buildSecp256r1PrecompileIx(
    params.signer.publicKeyBytes,
    precompileMessage,
    signature,
  );

  return { authPayload, precompileIx };
}

// ─── Data payload builders ──────────────────────────────────────────

/**
 * AddAuthority data payload:
 * [type(1)][role(1)][padding(6)][credential(32)][secp256r1Pubkey?(33)][rpIdLen?(1)][rpId?(N)]
 */
export function buildDataPayloadForAdd(
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
 * TransferOwnership data payload: [auth_type(1)][full_auth_data]
 */
export function buildDataPayloadForTransfer(
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
 * CreateSession data payload: [session_key(32)][expires_at(8)]
 */
export function buildDataPayloadForSession(
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

// ─── Helpers ────────────────────────────────────────────────────────

export function concatParts(parts: Uint8Array[]): Uint8Array {
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
 */
export function buildSecp256r1PrecompileIx(
  publicKey: Uint8Array,
  message: Uint8Array,
  signature: Uint8Array,
): TransactionInstruction {
  const SECP256R1_PROGRAM_ID = new PublicKey('Secp256r1SigVerify1111111111111111111111111');

  const HEADER_SIZE = 16;
  const sigOffset = HEADER_SIZE;
  const pubkeyOffset = sigOffset + 64;
  const msgOffset = pubkeyOffset + 33 + 1; // 1-byte alignment padding

  const data = Buffer.alloc(HEADER_SIZE + 64 + 33 + 1 + message.length);
  let off = 0;

  data.writeUInt8(1, off); off += 1;
  data.writeUInt8(0, off); off += 1;
  data.writeUInt16LE(sigOffset, off); off += 2;
  data.writeUInt16LE(0xFFFF, off); off += 2;
  data.writeUInt16LE(pubkeyOffset, off); off += 2;
  data.writeUInt16LE(0xFFFF, off); off += 2;
  data.writeUInt16LE(msgOffset, off); off += 2;
  data.writeUInt16LE(message.length, off); off += 2;
  data.writeUInt16LE(0xFFFF, off); off += 2;

  Buffer.from(signature).copy(data, sigOffset);
  Buffer.from(publicKey).copy(data, pubkeyOffset);
  Buffer.from(message).copy(data, msgOffset);

  return new TransactionInstruction({
    programId: SECP256R1_PROGRAM_ID,
    keys: [],
    data,
  });
}
