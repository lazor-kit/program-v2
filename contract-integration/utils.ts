import * as anchor from '@coral-xyz/anchor';

export function instructionToAccountMetas(
  ix: anchor.web3.TransactionInstruction
): anchor.web3.AccountMeta[] {
  return ix.keys.map((k) => ({
    pubkey: k.pubkey,
    isWritable: k.isWritable,
    isSigner: false,
  }));
}
export function getRandomBytes(len: number): Uint8Array {
  if (typeof globalThis.crypto?.getRandomValues === 'function') {
    const arr = new Uint8Array(len);
    globalThis.crypto.getRandomValues(arr);
    return arr;
  }
  try {
    // Node.js fallback
    const { randomBytes } = require('crypto');
    return randomBytes(len);
  } catch {
    throw new Error('No CSPRNG available');
  }
}
