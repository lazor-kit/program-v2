import * as crypto from 'crypto';
// @ts-ignore
import ECDSA from 'ecdsa-secp256r1';
import {
  PublicKey,
  TransactionInstruction,
} from '@solana/web3.js';
import {
  buildAuthPayload,
  buildAuthPayloadPrefix,
  buildSecp256r1Challenge,
  generateAuthenticatorData,
  type Secp256r1Signer,
  type WebAuthnResponse,
} from '@lazorkit/sdk-legacy';
import { PROGRAM_ID } from './common';

const SECP256R1_PROGRAM_ID = new PublicKey('Secp256r1SigVerify1111111111111111111111111');

// Secp256r1 curve order
const SECP256R1_N = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551n;
const HALF_N = SECP256R1_N / 2n;

export interface MockSecp256r1Key {
  privateKey: any;
  publicKeyBytes: Uint8Array;
  credentialIdHash: Uint8Array;
  rpId: string;
}

export async function generateMockSecp256r1Key(
  rpId = 'example.com',
  credentialIdHash?: Uint8Array,
): Promise<MockSecp256r1Key> {
  const privateKey = await ECDSA.generateKey();
  const pubKeyBase64 = privateKey.toCompressedPublicKey();
  const compressedPubKey = new Uint8Array(Buffer.from(pubKeyBase64, 'base64'));
  const credHash = credentialIdHash ?? crypto.randomBytes(32);

  return {
    privateKey,
    publicKeyBytes: compressedPubKey,
    credentialIdHash: new Uint8Array(credHash),
    rpId,
  };
}

function enforceLowS(rawSig: Uint8Array): Uint8Array {
  // Pad to 64 bytes if the library returned a shorter buffer
  if (rawSig.length < 64) {
    const padded = new Uint8Array(64);
    padded.set(rawSig, 64 - rawSig.length);
    rawSig = padded;
  }
  const sBytes = rawSig.slice(32, 64);
  let s = 0n;
  for (let i = 0; i < 32; i++) s = (s << 8n) + BigInt(sBytes[i]);

  if (s > HALF_N) {
    s = SECP256R1_N - s;
    for (let i = 31; i >= 0; i--) {
      sBytes[i] = Number(s & 0xffn);
      s >>= 8n;
    }
    rawSig.set(sBytes, 32);
  }
  return rawSig;
}

function bytesToBase64UrlNoPad(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

/**
 * Creates a Secp256r1Signer that returns raw clientDataJSON — the only
 * supported auth mode. Simulates a real browser authenticator.
 */
export function createMockRawSigner(key: MockSecp256r1Key): Secp256r1Signer {
  return {
    publicKeyBytes: key.publicKeyBytes,
    credentialIdHash: key.credentialIdHash,
    rpId: key.rpId,
    async sign(challenge: Uint8Array) {
      const authenticatorData = generateAuthenticatorData(key.rpId);

      const clientDataJson = JSON.stringify({
        type: 'webauthn.get',
        challenge: bytesToBase64UrlNoPad(challenge),
        origin: `https://${key.rpId}`,
        crossOrigin: false,
      });
      const clientDataJsonBytes = new Uint8Array(Buffer.from(clientDataJson, 'utf-8'));
      const clientDataJsonHash = new Uint8Array(
        crypto.createHash('sha256').update(clientDataJsonBytes).digest(),
      );

      const messageToSign = Buffer.concat([authenticatorData, clientDataJsonHash]);
      const signatureBase64 = await key.privateKey.sign(Buffer.from(messageToSign));
      const signature = enforceLowS(new Uint8Array(Buffer.from(signatureBase64, 'base64')));

      return { signature, authenticatorData, clientDataJsonHash, clientDataJson: clientDataJsonBytes };
    },
  };
}

/**
 * Simulates what navigator.credentials.get() returns.
 * Takes a challenge and returns a WebAuthn response — use this with the
 * prepare/finalize flow to fake the browser authenticator step.
 */
export async function fakeWebAuthnSign(
  key: MockSecp256r1Key,
  challenge: Uint8Array,
): Promise<WebAuthnResponse> {
  const authenticatorData = generateAuthenticatorData(key.rpId);

  const clientDataJson = JSON.stringify({
    type: 'webauthn.get',
    challenge: bytesToBase64UrlNoPad(challenge),
    origin: `https://${key.rpId}`,
    crossOrigin: false,
  });
  const clientDataJsonBytes = new Uint8Array(Buffer.from(clientDataJson, 'utf-8'));
  const clientDataJsonHash = new Uint8Array(
    crypto.createHash('sha256').update(clientDataJsonBytes).digest(),
  );

  const messageToSign = Buffer.concat([authenticatorData, clientDataJsonHash]);
  const signatureBase64 = await key.privateKey.sign(Buffer.from(messageToSign));
  const signature = enforceLowS(new Uint8Array(Buffer.from(signatureBase64, 'base64')));

  return { signature, authenticatorData, clientDataJsonHash, clientDataJson: clientDataJsonBytes };
}

/**
 * Full Secp256r1 signing flow for low-level tests.
 * Builds auth payload, computes challenge hash, signs it via WebAuthn-compatible
 * flow, and returns the precompile instruction + auth payload bytes.
 *
 * Use `createMockSigner()` + `LazorKitClient` for simpler tests.
 */
/**
 * Full Secp256r1 signing flow (raw clientDataJSON) for low-level tests.
 * Uses the 14-byte prefix for challenge computation, then builds the full
 * auth payload with authenticatorData + raw clientDataJSON.
 */
export async function signSecp256r1Raw(params: {
  key: MockSecp256r1Key;
  discriminator: Uint8Array;
  signedPayload: Uint8Array;
  slot: bigint;
  counter: number;
  payer: PublicKey;
  sysvarIxIndex: number;
  programId?: PublicKey;
}): Promise<{
  authPayload: Uint8Array;
  precompileIx: TransactionInstruction;
}> {
  const pid = params.programId ?? PROGRAM_ID;

  // Mode 1: Use only the 14-byte prefix for challenge computation
  const challengePrefix = buildAuthPayloadPrefix({
    slot: params.slot,
    counter: params.counter,
    sysvarIxIndex: params.sysvarIxIndex,
  });

  // Compute challenge hash with the prefix (not the full payload)
  const challengeHash = buildSecp256r1Challenge({
    discriminator: params.discriminator,
    authPayload: challengePrefix,
    signedPayload: params.signedPayload,
    slot: params.slot,
    payer: params.payer,
    counter: params.counter,
    programId: pid,
  });

  // Build authenticatorData and clientDataJSON
  const authenticatorData = generateAuthenticatorData(params.key.rpId);

  const clientDataJson = JSON.stringify({
    type: 'webauthn.get',
    challenge: bytesToBase64UrlNoPad(challengeHash),
    origin: `https://${params.key.rpId}`,
    crossOrigin: false,
  });
  const clientDataJsonBytes = new Uint8Array(Buffer.from(clientDataJson, 'utf-8'));
  const clientDataJsonHash = new Uint8Array(
    crypto.createHash('sha256').update(clientDataJsonBytes).digest(),
  );

  // Sign: authenticatorData || clientDataJsonHash
  const messageToSign = Buffer.concat([authenticatorData, clientDataJsonHash]);
  const signatureBase64 = await params.key.privateKey.sign(Buffer.from(messageToSign));
  const rawSig = enforceLowS(new Uint8Array(Buffer.from(signatureBase64, 'base64')));

  // Build full auth payload
  const authPayload = buildAuthPayload({
    slot: params.slot,
    counter: params.counter,
    sysvarIxIndex: params.sysvarIxIndex,
    authenticatorData,
    clientDataJson: clientDataJsonBytes,
  });

  // Build precompile instruction
  const precompileIx = buildPrecompileIx(
    params.key.publicKeyBytes,
    new Uint8Array(messageToSign),
    rawSig,
  );

  return { authPayload, precompileIx };
}

function buildPrecompileIx(
  publicKey: Uint8Array,
  message: Uint8Array,
  signature: Uint8Array,
): TransactionInstruction {
  // Layout must match on-chain introspection.rs constants:
  //   DATA_START = 16, SIGNATURE_DATA_OFFSET = 16, PUBKEY_DATA_OFFSET = 80
  //   MESSAGE_DATA_OFFSET = 114 (80 + 33 + 1 alignment padding)
  const HEADER_SIZE = 16;
  const sigOffset = HEADER_SIZE;          // 16
  const pubkeyOffset = sigOffset + 64;    // 80
  const msgOffset = pubkeyOffset + 33 + 1; // 114 (1-byte alignment padding after pubkey)

  const data = Buffer.alloc(HEADER_SIZE + 64 + 33 + 1 + message.length);
  let off = 0;

  data.writeUInt8(1, off); off += 1;                // num_signatures
  data.writeUInt8(0, off); off += 1;                // padding
  data.writeUInt16LE(sigOffset, off); off += 2;     // sig_offset
  data.writeUInt16LE(0xffff, off); off += 2;        // sig_ix_index
  data.writeUInt16LE(pubkeyOffset, off); off += 2;  // pubkey_offset
  data.writeUInt16LE(0xffff, off); off += 2;        // pubkey_ix_index
  data.writeUInt16LE(msgOffset, off); off += 2;     // msg_offset
  data.writeUInt16LE(message.length, off); off += 2; // msg_size
  data.writeUInt16LE(0xffff, off); off += 2;        // msg_ix_index

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

// ─── Backwards-compat aliases (program-v2 tests use older names) ─────────
export const createMockSigner = createMockRawSigner;
export const signSecp256r1 = signSecp256r1Raw;
