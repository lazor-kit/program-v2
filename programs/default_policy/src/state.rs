use anchor_lang::prelude::*;

#[derive(Debug, AnchorSerialize, AnchorDeserialize, InitSpace)]
pub struct PolicyStruct {
    pub smart_wallet: Pubkey,
    #[max_len(5)]
    pub authoritis: Vec<Pubkey>, // max 5
}

impl PolicyStruct {
    pub const LEN: usize = PolicyStruct::INIT_SPACE;
}
