use anchor_lang::prelude::*;

/// Data account for a smart wallet
#[account]
#[derive(Default, InitSpace)]
pub struct SmartWallet {
    /// Unique identifier for this smart wallet
    pub id: u64,
    /// Referral address that governs this wallet's operations
    pub referral: Pubkey,
    /// Policy program that governs this wallet's operations
    pub policy_program: Pubkey,
    /// Last nonce used for message verification
    pub last_nonce: u64,
    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl SmartWallet {
    pub const PREFIX_SEED: &'static [u8] = b"smart_wallet_data";
}
