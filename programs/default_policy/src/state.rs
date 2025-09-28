use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct Policy {
    pub smart_wallet: Pubkey,
    /// List of wallet devices associated with the smart wallet
    #[max_len(10)]
    pub list_wallet_device: Vec<Pubkey>,
}

impl Policy {
    pub const PREFIX_SEED: &'static [u8] = b"policy";
}
