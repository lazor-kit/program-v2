/**
 * PDA derivation helpers for LazorKit accounts.
 *
 * Uses @solana/web3.js v1 PublicKey.findProgramAddressSync().
 * Same seed patterns as the codama-client v2 version.
 */

import { PublicKey } from '@solana/web3.js';

export const PROGRAM_ID = new PublicKey(
  'FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao',
);

/**
 * Derives the Wallet PDA.
 * Seeds: ["wallet", user_seed]
 */
export function findWalletPda(
  userSeed: Uint8Array,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('wallet'), userSeed],
    programId,
  );
}

/**
 * Derives the Vault PDA.
 * Seeds: ["vault", wallet_pubkey]
 */
export function findVaultPda(
  wallet: PublicKey,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('vault'), wallet.toBuffer()],
    programId,
  );
}

/**
 * Derives an Authority PDA.
 * Seeds: ["authority", wallet_pubkey, id_seed]
 * @param idSeed - For Ed25519 this is the 32-byte public key.
 *                 For Secp256r1 this is the 32-byte SHA256 Hash of the credential_id (rawId).
 */
export function findAuthorityPda(
  wallet: PublicKey,
  idSeed: Uint8Array,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('authority'), wallet.toBuffer(), idSeed],
    programId,
  );
}

/**
 * Derives a Session PDA.
 * Seeds: ["session", wallet_pubkey, session_key_pubkey]
 */
export function findSessionPda(
  wallet: PublicKey,
  sessionKey: PublicKey,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('session'), wallet.toBuffer(), sessionKey.toBuffer()],
    programId,
  );
}

/**
 * Derives the Config PDA.
 * Seeds: ["config"]
 */
export function findConfigPda(
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([Buffer.from('config')], programId);
}

/**
 * Derives a Treasury Shard PDA.
 * Seeds: ["treasury", shard_id]
 */
export function findTreasuryShardPda(
  shardId: number,
  programId: PublicKey = PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from('treasury'), new Uint8Array([shardId])],
    programId,
  );
}
