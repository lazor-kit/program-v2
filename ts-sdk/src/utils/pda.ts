import {
  Address,
  getAddressEncoder,
  getProgramDerivedAddress,
} from '@solana/kit';
import { LazorkitError, LazorkitErrorCode } from '../errors';
import { WALLET_ACCOUNT_PREFIX, WALLET_VAULT_PREFIX, assertIsWalletId, assertIsAddress } from '../types';

/**
 * Lazorkit V2 Program ID (as Address)
 * 
 * Mainnet: BAXwCwbBbs5WmdUkG9EEtFoLsYq2vRADBkdShbRN7w1P
 */
export const LAZORKIT_PROGRAM_ID: Address = 'CmF46cm89WjdfCDDDTx5X2kQLc2mFVUhP3k7k3txgAFE' as Address;

/**
 * Find wallet account PDA
 * 
 * Seeds: [b"wallet_account", wallet_id]
 * 
 * @param walletId - 32-byte wallet identifier
 * @param programId - Optional program ID (defaults to LAZORKIT_PROGRAM_ID)
 * @returns [Address, bump] - The PDA address and bump seed
 */
export async function findWalletAccount(
  walletId: Uint8Array,
  programId: Address = LAZORKIT_PROGRAM_ID
): Promise<[Address, number]> {
  // Validate inputs
  assertIsWalletId(walletId);
  assertIsAddress(programId);

  const seeds = [
    new TextEncoder().encode(WALLET_ACCOUNT_PREFIX),
    walletId,
  ];

  console.log('[PDA] findWalletAccount called:');
  console.log('  - programId:', programId);
  console.log('  - WALLET_ACCOUNT_PREFIX:', WALLET_ACCOUNT_PREFIX);
  console.log('  - walletId (hex):', Buffer.from(walletId).toString('hex'));
  console.log('  - walletId (length):', walletId.length);

  try {
    const [address, bump] = await getProgramDerivedAddress({
      programAddress: programId,
      seeds,
    });
    // Convert branded ProgramDerivedAddressBump to number
    return [address, Number(bump)];
  } catch (error) {
    throw new LazorkitError(
      LazorkitErrorCode.PdaDerivationFailed,
      `Failed to derive wallet account PDA: ${error instanceof Error ? error.message : String(error)}`,
      error instanceof Error ? error : undefined
    );
  }
}

/**
 * Find wallet vault PDA
 * 
 * Seeds: [b"wallet_vault", wallet_account_address]
 * 
 * @param walletAccount - The wallet account address
 * @param programId - Optional program ID (defaults to LAZORKIT_PROGRAM_ID)
 * @returns [Address, bump] - The PDA address and bump seed
 */
export async function findWalletVault(
  walletAccount: Address,
  programId: Address = LAZORKIT_PROGRAM_ID
): Promise<[Address, number]> {
  // Validate addresses
  assertIsAddress(walletAccount);
  assertIsAddress(programId);

  const addressEncoder = getAddressEncoder();
  const walletAccountBytes = addressEncoder.encode(walletAccount);

  const seeds = [
    new TextEncoder().encode(WALLET_VAULT_PREFIX),
    walletAccountBytes,
  ];

  try {
    const [address, bump] = await getProgramDerivedAddress({
      programAddress: programId,
      seeds,
    });
    // Convert branded ProgramDerivedAddressBump to number
    return [address, Number(bump)];
  } catch (error) {
    throw new LazorkitError(
      LazorkitErrorCode.PdaDerivationFailed,
      `Failed to derive wallet vault PDA: ${error instanceof Error ? error.message : String(error)}`,
      error instanceof Error ? error : undefined
    );
  }
}

/**
 * Create wallet account signer seeds
 * 
 * Used for signing transactions with the wallet account PDA
 * 
 * @param walletId - 32-byte wallet identifier
 * @param bump - Bump seed for the PDA
 * @returns Array of seed Uint8Arrays
 */
export function createWalletAccountSignerSeeds(
  walletId: Uint8Array,
  bump: number
): Uint8Array[] {
  // Validate inputs
  assertIsWalletId(walletId);

  // Validate bump is in valid range [0, 255]
  if (bump < 0 || bump > 255) {
    throw new LazorkitError(
      LazorkitErrorCode.PdaDerivationFailed,
      `Invalid bump seed: ${bump}. Must be between 0 and 255`
    );
  }

  return [
    new TextEncoder().encode(WALLET_ACCOUNT_PREFIX),
    new Uint8Array(walletId),
    new Uint8Array([bump]),
  ];
}

/**
 * Create wallet vault signer seeds
 * 
 * Used for signing transactions with the wallet vault PDA
 * 
 * @param walletAccount - The wallet account address
 * @param bump - Bump seed for the PDA
 * @returns Array of seed Uint8Arrays
 */
export function createWalletVaultSignerSeeds(
  walletAccount: Address,
  bump: number
): Uint8Array[] {
  assertIsAddress(walletAccount);

  const addressEncoder = getAddressEncoder();
  const walletAccountBytes = addressEncoder.encode(walletAccount);

  return [
    new TextEncoder().encode(WALLET_VAULT_PREFIX),
    new Uint8Array(walletAccountBytes),
    new Uint8Array([bump]),
  ];
}
