use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct Policy {
    pub smart_wallet: Pubkey,
    pub wallet_device: Pubkey,
}
