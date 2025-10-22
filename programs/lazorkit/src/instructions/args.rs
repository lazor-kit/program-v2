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
    pub vault_index: u8,
    pub timestamp: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ChangePolicyArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub destroy_policy_data: Vec<u8>,
    pub init_policy_data: Vec<u8>,
    pub vault_index: u8,
    pub timestamp: i64,
    pub new_wallet_device: Option<NewWalletDeviceArgs>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CallPolicyArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub policy_data: Vec<u8>,
    pub new_wallet_device: Option<NewWalletDeviceArgs>,
    pub vault_index: u8,
    pub timestamp: i64,
    pub smart_wallet_is_signer: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateChunkArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub policy_data: Vec<u8>,
    pub vault_index: u8,
    pub timestamp: i64,
    pub cpi_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct NewWalletDeviceArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub credential_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct GrantPermissionArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub ephemeral_public_key: Pubkey,
    pub expires_at: i64,
    pub vault_index: u8,
    pub instruction_data_list: Vec<Vec<u8>>,
    pub split_index: Vec<u8>,
    pub timestamp: i64,
}
