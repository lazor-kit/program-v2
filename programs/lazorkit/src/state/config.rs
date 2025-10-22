use anchor_lang::prelude::*;

/// LazorKit program configuration and settings
///
/// Stores global program configuration including fee structures, default policy
/// program, and operational settings. Only the program authority can modify
/// these settings through the update_config instruction.
///
/// Memory layout optimized for better cache performance:
/// - Group related fields together
/// - Align fields to natural boundaries
#[account]
#[derive(Default, InitSpace)]
pub struct Config {
    pub is_paused: bool,
    pub create_smart_wallet_fee: u64,
    pub fee_payer_fee: u64,
    pub referral_fee: u64,
    pub lazorkit_fee: u64,
    pub authority: Pubkey,
    pub default_policy_program_id: Pubkey,
}

impl Config {
    /// Seed prefix used for PDA derivation of the config account
    pub const PREFIX_SEED: &'static [u8] = b"config";
}

/// Types of configuration parameters that can be updated
///
/// Defines all the configuration parameters that can be modified through
/// the update_config instruction by the program authority.
#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub enum UpdateType {
    /// Update the fee charged for creating smart wallets
    CreateWalletFee = 0,
    /// Update the fee charged to transaction fee payers
    FeePayerFee = 1,
    /// Update the fee paid to referral addresses
    ReferralFee = 2,
    /// Update the fee retained by LazorKit protocol
    LazorkitFee = 3,
    /// Update the default policy program for new wallets
    DefaultPolicyProgram = 4,
    /// Update the program authority
    Admin = 5,
    /// Pause the program (emergency stop)
    PauseProgram = 6,
    /// Unpause the program (resume operations)
    UnpauseProgram = 7,
}
