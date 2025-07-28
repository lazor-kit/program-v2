use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct Rule {
    pub smart_wallet: Pubkey,
    pub smart_wallet_authenticator: Pubkey,
}
