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
/// - Minimize padding
#[account]
#[derive(Default, InitSpace)]
pub struct Config {
    /// Whether the program is currently paused (1 byte)
    pub is_paused: bool,
    /// Padding to align next fields (7 bytes)
    pub _padding: [u8; 7],
    /// Fee charged for creating a new smart wallet (in lamports) (8 bytes)
    pub create_smart_wallet_fee: u64,
    /// Fee charged to the fee payer for transactions (in lamports) (8 bytes)
    pub fee_payer_fee: u64,
    /// Fee paid to referral addresses (in lamports) (8 bytes)
    pub referral_fee: u64,
    /// Fee retained by LazorKit protocol (in lamports) (8 bytes)
    pub lazorkit_fee: u64,
    /// Program authority that can modify configuration settings (32 bytes)
    pub authority: Pubkey,
    /// Default policy program ID for new smart wallets (32 bytes)
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
