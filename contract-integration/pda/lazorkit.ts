import { PublicKey } from '@solana/web3.js';
import { BN } from '@coral-xyz/anchor';
import { Buffer } from 'buffer';
import { createWalletDeviceHash } from '../webauthn/secp256r1';
// Mirror on-chain seeds

export const SMART_WALLET_SEED = Buffer.from('smart_wallet');
export const SMART_WALLET_CONFIG_SEED = Buffer.from('wallet_state');
export const WALLET_DEVICE_SEED = Buffer.from('wallet_device');
export const CHUNK_SEED = Buffer.from('chunk');
export const PERMISSION_SEED = Buffer.from('permission');

export function deriveSmartWalletPda(
  programId: PublicKey,
  walletId: BN
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [SMART_WALLET_SEED, walletId.toArrayLike(Buffer, 'le', 8)],
    programId
  )[0];
}

export function deriveSmartWalletConfigPda(
  programId: PublicKey,
  smartWallet: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [SMART_WALLET_CONFIG_SEED, smartWallet.toBuffer()],
    programId
  )[0];
}

export function deriveWalletDevicePda(
  programId: PublicKey,
  smartWallet: PublicKey,
  credentialHash: number[]
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [WALLET_DEVICE_SEED, createWalletDeviceHash(smartWallet, credentialHash)],
    programId
  );
}

export function deriveChunkPda(
  programId: PublicKey,
  smartWallet: PublicKey,
  lastNonce: BN
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [
      CHUNK_SEED,
      smartWallet.toBuffer(),
      lastNonce.toArrayLike(Buffer, 'le', 8),
    ],
    programId
  )[0];
}

export function derivePermissionPda(
  programId: PublicKey,
  smartWallet: PublicKey,
  ephemeralPublicKey: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [PERMISSION_SEED, smartWallet.toBuffer(), ephemeralPublicKey.toBuffer()],
    programId
  )[0];
}
