import { PublicKey } from '@solana/web3.js';
import { PROGRAM_ID } from '../generated';

export function findWalletPda(
  userSeed: Uint8Array,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('wallet'), userSeed],
    programId,
  );
}

export function findVaultPda(
  walletPda: PublicKey,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('vault'), walletPda.toBuffer()],
    programId,
  );
}

export function findAuthorityPda(
  walletPda: PublicKey,
  credentialIdHash: Uint8Array,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('authority'), walletPda.toBuffer(), credentialIdHash],
    programId,
  );
}

export function findSessionPda(
  walletPda: PublicKey,
  sessionKey: Uint8Array,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('session'), walletPda.toBuffer(), sessionKey],
    programId,
  );
}

export function findDeferredExecPda(
  walletPda: PublicKey,
  authorityPda: PublicKey,
  counter: number,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  const counterBuf = Buffer.alloc(4);
  counterBuf.writeUInt32LE(counter);
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from('deferred'),
      walletPda.toBuffer(),
      authorityPda.toBuffer(),
      counterBuf,
    ],
    programId,
  );
}
