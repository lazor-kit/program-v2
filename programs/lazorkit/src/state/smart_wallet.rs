use anchor_lang::prelude::*;

/// Core data account for a LazorKit smart wallet
///
/// Stores the essential state information for a smart wallet including its
/// unique identifier, policy program configuration, and authentication nonce
/// for replay attack prevention.
///
/// Memory layout optimized for better cache performance:
/// - Group related fields together
/// - Align fields to natural boundaries
/// - Minimize padding
#[account]
#[derive(Default, InitSpace)]
pub struct SmartWalletConfig {
    /// Bump seed for PDA derivation and verification (1 byte)
    pub bump: u8,
    /// Unique identifier for this smart wallet instance (8 bytes)
    pub wallet_id: u64,
    /// Last nonce used for message verification to prevent replay attacks (8 bytes)
    pub last_nonce: u64,
    /// Referral address that receives referral fees from this wallet (32 bytes)
    pub referral_address: Pubkey,
    /// Policy program that governs this wallet's transaction validation rules (32 bytes)
    pub policy_program_id: Pubkey,
}

impl SmartWalletConfig {
    /// Seed prefix used for PDA derivation of smart wallet data accounts
    pub const PREFIX_SEED: &'static [u8] = b"smart_wallet_config";
}
