use anchor_lang::prelude::*;

/// Core data account for a LazorKit smart wallet
/// 
/// This account stores the essential state information for a smart wallet,
/// including its unique identifier, policy program, and authentication nonce.
#[account]
#[derive(Default, InitSpace)]
pub struct SmartWalletData {
    /// Unique identifier for this smart wallet instance
    pub wallet_id: u64,
    /// Referral address that receives referral fees from this wallet
    pub referral_address: Pubkey,
    /// Policy program that governs this wallet's transaction validation rules
    pub policy_program_id: Pubkey,
    /// Last nonce used for message verification to prevent replay attacks
    pub last_nonce: u64,
    /// Bump seed for PDA derivation and verification
    pub bump: u8,
}

impl SmartWalletData {
    /// Seed prefix used for PDA derivation of smart wallet data accounts
    pub const PREFIX_SEED: &'static [u8] = b"smart_wallet_data";
}
