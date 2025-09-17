import { PublicKey } from '@solana/web3.js';
import { BN } from '@coral-xyz/anchor';
import { Buffer } from 'buffer';
// Mirror on-chain seeds
export const CONFIG_SEED = Buffer.from('config');
export const POLICY_PROGRAM_REGISTRY_SEED = Buffer.from('policy_registry');
export const SMART_WALLET_SEED = Buffer.from('smart_wallet');
export const SMART_WALLET_CONFIG_SEED = Buffer.from('smart_wallet_config');
export const WALLET_DEVICE_SEED = Buffer.from('wallet_device');
export const CHUNK_SEED = Buffer.from('chunk');
export const PERMISSION_SEED = Buffer.from('permission');
export const LAZORKIT_VAULT_SEED = Buffer.from('lazorkit_vault');

export function deriveConfigPda(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([CONFIG_SEED], programId)[0];
}

export function derivePolicyProgramRegistryPda(
  programId: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [POLICY_PROGRAM_REGISTRY_SEED],
    programId
  )[0];
}

export function deriveLazorkitVaultPda(
  programId: PublicKey,
  index: number
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [LAZORKIT_VAULT_SEED, Buffer.from([index])],
    programId
  )[0];
}

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

// Must match on-chain: sha256(passkey(33) || wallet(32))
export function hashPasskeyWithWallet(
  passkeyCompressed33: number[],
  wallet: PublicKey
): Buffer {
  const { sha256 } = require('js-sha256');
  const buf = Buffer.alloc(65);
  Buffer.from(passkeyCompressed33).copy(buf, 0);
  wallet.toBuffer().copy(buf, 33);
  return Buffer.from(sha256.arrayBuffer(buf)).subarray(0, 32);
}

export function deriveWalletDevicePda(
  programId: PublicKey,
  smartWallet: PublicKey,
  passkeyCompressed33: number[]
): [PublicKey, number] {
  const hashed = hashPasskeyWithWallet(passkeyCompressed33, smartWallet);
  return PublicKey.findProgramAddressSync(
    [WALLET_DEVICE_SEED, smartWallet.toBuffer(), hashed],
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
