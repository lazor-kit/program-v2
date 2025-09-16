use anchor_lang::prelude::*;

/// Data account for a smart wallet
#[account]
#[derive(Default, InitSpace)]
pub struct SmartWalletData {
    /// Unique identifier for this smart wallet
    pub wallet_id: u64,
    /// Referral address that governs this wallet's operations
    pub referral_address: Pubkey,
    /// Policy program that governs this wallet's operations
    pub policy_program_id: Pubkey,
    /// Last nonce used for message verification
    pub last_nonce: u64,
    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl SmartWalletData {
    pub const PREFIX_SEED: &'static [u8] = b"smart_wallet_data";
}
