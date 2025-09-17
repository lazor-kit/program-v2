use anchor_lang::prelude::*;

/// LazorKit program constants and configuration values
/// 
/// Contains all constant values used throughout the LazorKit program including
/// program IDs, seed values, size constraints, and configuration parameters.

/// Solana's built-in Secp256r1 signature verification program ID
pub const SECP256R1_PROGRAM_ID: Pubkey = pubkey!("Secp256r1SigVerify1111111111111111111111111");

/// Seed used for smart wallet PDA derivation
pub const SMART_WALLET_SEED: &[u8] = b"smart_wallet";

/// Size of a Secp256r1 compressed public key in bytes
pub const PASSKEY_PUBLIC_KEY_SIZE: usize = 33;

/// Minimum rent-exempt balance for empty PDA accounts (in lamports)
pub const EMPTY_PDA_RENT_EXEMPT_BALANCE: u64 = 890880;