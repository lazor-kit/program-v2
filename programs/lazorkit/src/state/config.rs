use anchor_lang::prelude::*;

/// LazorKit program configuration and settings
///
/// Stores global program configuration including fee structures, default policy
/// program, and operational settings. Only the program authority can modify
/// these settings through the update_config instruction.
#[account]
#[derive(Default, InitSpace)]
pub struct Config {
    /// Program authority that can modify configuration settings
    pub authority: Pubkey,
    /// Fee charged for creating a new smart wallet (in lamports)
    pub create_smart_wallet_fee: u64,
    /// Fee charged to the fee payer for transactions (in lamports)
    pub fee_payer_fee: u64,
    /// Fee paid to referral addresses (in lamports)
    pub referral_fee: u64,
    /// Fee retained by LazorKit protocol (in lamports)
    pub lazorkit_fee: u64,
    /// Default policy program ID for new smart wallets
    pub default_policy_program_id: Pubkey,
    /// Whether the program is currently paused
    pub is_paused: bool,
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
