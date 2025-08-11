import * as anchor from '@coral-xyz/anchor';
import { PublicKey, TransactionInstruction } from '@solana/web3.js';

const SIGNATURE_OFFSETS_SERIALIZED_SIZE = 14;
const SIGNATURE_OFFSETS_START = 2;
const DATA_START = SIGNATURE_OFFSETS_SERIALIZED_SIZE + SIGNATURE_OFFSETS_START;
const SIGNATURE_SERIALIZED_SIZE = 64;
const COMPRESSED_PUBKEY_SERIALIZED_SIZE = 33;
const FIELD_SIZE = 32;

export const SECP256R1_PROGRAM_ID = new PublicKey('Secp256r1SigVerify1111111111111111111111111');

const ORDER = new Uint8Array([
  0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
  0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
]);
const HALF_ORDER = new Uint8Array([
  0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
  0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
]);

function isGreaterThan(a: Uint8Array, b: Uint8Array): boolean {
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return a[i] > b[i];
  }
  return false;
}

function sub(a: Uint8Array, b: Uint8Array): Uint8Array {
  const out = new Uint8Array(a.length);
  let borrow = 0;
  for (let i = a.length - 1; i >= 0; i--) {
    let d = a[i] - b[i] - borrow;
    if (d < 0) {
      d += 256;
      borrow = 1;
    } else {
      borrow = 0;
    }
    out[i] = d;
  }
  return out;
}

function bytesOf(obj: any): Uint8Array {
  if (obj instanceof Uint8Array) return obj;
  if (Array.isArray(obj)) return new Uint8Array(obj);
  const keys = Object.keys(obj);
  const buf = new ArrayBuffer(keys.length * 2);
  const view = new DataView(buf);
  keys.forEach((k, i) => view.setUint16(i * 2, obj[k] as number, true));
  return new Uint8Array(buf);
}

export function buildSecp256r1VerifyIx(
  message: Uint8Array,
  compressedPubkey33: Uint8Array,
  signature: Uint8Array
): TransactionInstruction {
  let sig = Buffer.from(signature);
  if (sig.length !== SIGNATURE_SERIALIZED_SIZE) {
    const r = sig.subarray(0, FIELD_SIZE);
    const s = sig.subarray(FIELD_SIZE, FIELD_SIZE * 2);
    const R = Buffer.alloc(FIELD_SIZE);
    const S = Buffer.alloc(FIELD_SIZE);
    r.copy(R, FIELD_SIZE - r.length);
    s.copy(S, FIELD_SIZE - s.length);
    if (isGreaterThan(S, HALF_ORDER)) {
      const newS = sub(ORDER, S);
      sig = Buffer.concat([R, Buffer.from(newS)]);
    } else {
      sig = Buffer.concat([R, S]);
    }
  }

  if (
    compressedPubkey33.length !== COMPRESSED_PUBKEY_SERIALIZED_SIZE ||
    sig.length !== SIGNATURE_SERIALIZED_SIZE
  ) {
    throw new Error('Invalid secp256r1 key/signature length');
  }

  const totalSize =
    DATA_START + SIGNATURE_SERIALIZED_SIZE + COMPRESSED_PUBKEY_SERIALIZED_SIZE + message.length;
  const data = new Uint8Array(totalSize);

  const numSignatures = 1;
  const publicKeyOffset = DATA_START;
  const signatureOffset = publicKeyOffset + COMPRESSED_PUBKEY_SERIALIZED_SIZE;
  const messageDataOffset = signatureOffset + SIGNATURE_SERIALIZED_SIZE;

  data.set(bytesOf([numSignatures, 0]), 0);
  const offsets = {
    signature_offset: signatureOffset,
    signature_instruction_index: 0xffff,
    public_key_offset: publicKeyOffset,
    public_key_instruction_index: 0xffff,
    message_data_offset: messageDataOffset,
    message_data_size: message.length,
    message_instruction_index: 0xffff,
  } as const;
  data.set(bytesOf(offsets), SIGNATURE_OFFSETS_START);
  data.set(compressedPubkey33, publicKeyOffset);
  data.set(sig, signatureOffset);
  data.set(message, messageDataOffset);

  return new anchor.web3.TransactionInstruction({
    programId: SECP256R1_PROGRAM_ID,
    keys: [],
    data: Buffer.from(data),
  });
}
