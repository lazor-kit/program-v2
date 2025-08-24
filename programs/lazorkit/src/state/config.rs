use anchor_lang::prelude::*;

#[account]
#[derive(Default, InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub create_smart_wallet_fee: u64,
    pub execute_fee: u64,
    pub default_policy_program: Pubkey,
    pub is_paused: bool,
}

impl Config {
    pub const PREFIX_SEED: &'static [u8] = b"config";
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub enum UpdateConfigType {
    CreateWalletFee = 0,
    ExecuteFee = 1,
    DefaultPolicyProgram = 2,
    Admin = 3,
    PauseProgram = 4,
    UnpauseProgram = 5,
}
