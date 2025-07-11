use anchor_lang::prelude::*;

#[derive(Default, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct Message {
    pub nonce: u64,
    pub current_timestamp: i64,
    pub split_index: u16,
    pub rule_data: Vec<u8>,
    pub cpi_data: Vec<u8>,
}
