use anchor_lang::prelude::*;

// Constants from solana-secp256r1-program (defined locally to avoid crate compilation issues)
pub const COMPRESSED_PUBKEY_SERIALIZED_SIZE: usize = 33;
pub const SIGNATURE_SERIALIZED_SIZE: usize = 64;
pub const SIGNATURE_OFFSETS_SERIALIZED_SIZE: usize = 14;
pub const SIGNATURE_OFFSETS_START: usize = 2;
pub const DATA_START: usize = SIGNATURE_OFFSETS_SERIALIZED_SIZE + SIGNATURE_OFFSETS_START;

/// Solana's built-in Secp256r1 signature verification program ID
/// This is the program ID for the secp256r1 native program
pub const SECP256R1_PROGRAM_ID: Pubkey = anchor_lang::solana_program::pubkey!("Secp256r1SigVerify1111111111111111111111111");

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
