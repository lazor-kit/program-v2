use anchor_lang::prelude::*;

#[account]
#[derive(Default, InitSpace)]
pub struct ProgramConfig {
    pub authority: Pubkey,
    pub create_smart_wallet_fee: u64,
    pub fee_payer_fee: u64,
    pub referral_fee: u64,
    pub lazorkit_fee: u64,
    pub default_policy_program_id: Pubkey,
    pub is_paused: bool,
}

impl ProgramConfig {
    pub const PREFIX_SEED: &'static [u8] = b"config";
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub enum ConfigUpdateType {
    CreateWalletFee = 0,
    FeePayerFee = 1,
    ReferralFee = 2,
    LazorkitFee = 3,
    DefaultPolicyProgram = 4,
    Admin = 5,
    PauseProgram = 6,
    UnpauseProgram = 7,
}
