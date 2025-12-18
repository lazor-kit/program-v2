use anchor_lang::prelude::*;
use solana_secp256r1_program::COMPRESSED_PUBKEY_SERIALIZED_SIZE;

/// Solana's built-in Secp256r1 signature verification program ID
pub const SECP256R1_PROGRAM_ID: Pubkey = Pubkey::new_from_array(solana_secp256r1_program::ID.to_bytes());

/// Seed used for smart wallet PDA derivation
pub const SMART_WALLET_SEED: &[u8] = b"smart_wallet";

/// Size of a Secp256r1 compressed public key in bytes
pub const PASSKEY_PUBLIC_KEY_SIZE: usize = COMPRESSED_PUBKEY_SERIALIZED_SIZE;

/// Minimum rent-exempt balance for empty PDA accounts (in lamports)
/// Rationale: Based on Solana's current rent calculation for empty accounts
pub const EMPTY_PDA_RENT_EXEMPT_BALANCE: u64 = 890880;

/// Secp256r1 public key format constants
pub const SECP256R1_COMPRESSED_PUBKEY_PREFIX_EVEN: u8 = 0x02;
pub const SECP256R1_COMPRESSED_PUBKEY_PREFIX_ODD: u8 = 0x03;

/// Maximum instruction index for Secp256r1 verification
pub const MAX_VERIFY_INSTRUCTION_INDEX: u8 = 255;
