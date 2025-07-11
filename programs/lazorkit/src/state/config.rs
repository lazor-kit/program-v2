use anchor_lang::prelude::*;

#[account]
#[derive(Default, InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub create_smart_wallet_fee: u64,
    pub execute_fee: u64,
    pub default_rule_program: Pubkey,
}

impl Config {
    pub const PREFIX_SEED: &'static [u8] = b"config";
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub enum UpdateConfigType {
    CreateWalletFee = 0,
    ExecuteFee = 1,
    DefaultRuleProgram = 2,
    Admin = 3,
}
