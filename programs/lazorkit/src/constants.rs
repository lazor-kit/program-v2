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
/// Rationale: Based on Solana's current rent calculation for empty accounts
pub const EMPTY_PDA_RENT_EXEMPT_BALANCE: u64 = 890880;

/// Default fee configuration constants
pub const DEFAULT_FEE_PAYER_FEE: u64 = 30000; // 0.00003 SOL
pub const DEFAULT_REFERRAL_FEE: u64 = 10000;  // 0.00001 SOL
pub const DEFAULT_LAZORKIT_FEE: u64 = 10000;  // 0.00001 SOL

/// Maximum fee limits for validation
pub const MAX_CREATE_WALLET_FEE: u64 = 1_000_000_000; // 1 SOL
pub const MAX_TRANSACTION_FEE: u64 = 100_000_000;     // 0.1 SOL

/// Secp256r1 public key format constants
pub const SECP256R1_COMPRESSED_PUBKEY_PREFIX_EVEN: u8 = 0x02;
pub const SECP256R1_COMPRESSED_PUBKEY_PREFIX_ODD: u8 = 0x03;

/// Maximum instruction index for Secp256r1 verification
pub const MAX_VERIFY_INSTRUCTION_INDEX: u8 = 255;