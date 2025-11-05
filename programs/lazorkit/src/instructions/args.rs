use crate::constants::PASSKEY_PUBLIC_KEY_SIZE;
use anchor_lang::prelude::*;

pub trait Args {
    fn validate(&self) -> Result<()>;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub policy_data: Vec<u8>,
    pub cpi_data: Vec<u8>,
    pub timestamp: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateChunkArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub policy_data: Vec<u8>,
    pub timestamp: i64,
    pub cpi_hash: [u8; 32],
}
