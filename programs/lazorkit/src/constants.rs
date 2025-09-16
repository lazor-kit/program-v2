use anchor_lang::prelude::*;

/// Program IDs
pub const SECP256R1_ID: Pubkey = pubkey!("Secp256r1SigVerify1111111111111111111111111");

/// Seeds for PDA derivation
pub const SMART_WALLET_SEED: &[u8] = b"smart_wallet";

/// Size constants for account data
pub const PASSKEY_SIZE: usize = 33; // Secp256r1 compressed pubkey size

pub const EMPTY_PDA_FEE_RENT: u64 = 890880;