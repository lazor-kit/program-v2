use crate::{constants::PASSKEY_SIZE, error::LazorKitError};
use anchor_lang::prelude::*;

pub trait Args {
    fn validate(&self) -> Result<()>;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateSmartWalletArgs {
    pub passkey_pubkey: [u8; PASSKEY_SIZE],
    pub credential_id: Vec<u8>,
    pub policy_data: Vec<u8>,
    pub wallet_id: u64, // Random ID provided by client,
    pub is_pay_for_user: bool,
    pub referral: Option<Pubkey>,
    pub vault_index: u8, // Random vault index (0-31) calculated off-chain
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteTransactionArgs {
    pub passkey_pubkey: [u8; PASSKEY_SIZE],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub policy_data: Vec<u8>,
    pub cpi_data: Vec<u8>,
    pub vault_index: u8, // Random vault index (0-31) calculated off-chain
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdatePolicyArgs {
    pub passkey_pubkey: [u8; PASSKEY_SIZE],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub destroy_policy_data: Vec<u8>,
    pub init_policy_data: Vec<u8>,
    pub new_wallet_device: Option<NewWalletDeviceArgs>,
    pub vault_index: u8, // Random vault index (0-31) calculated off-chain
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InvokePolicyArgs {
    pub passkey_pubkey: [u8; PASSKEY_SIZE],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub policy_data: Vec<u8>,
    pub new_wallet_device: Option<NewWalletDeviceArgs>,
    pub vault_index: u8, // Random vault index (0-31) calculated off-chain
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateSessionArgs {
    pub passkey_pubkey: [u8; PASSKEY_SIZE],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub policy_data: Vec<u8>,
    pub expires_at: i64,
    pub vault_index: u8, // Random vault index (0-31) calculated off-chain
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct NewWalletDeviceArgs {
    pub passkey_pubkey: [u8; PASSKEY_SIZE],
    #[max_len(256)]
    pub credential_id: Vec<u8>,
}

macro_rules! impl_args_validate {
    ($t:ty) => {
        impl Args for $t {
            fn validate(&self) -> Result<()> {
                // Validate passkey format
                require!(
                    self.passkey_pubkey[0] == 0x02 || self.passkey_pubkey[0] == 0x03,
                    LazorKitError::InvalidPasskeyFormat
                );

                // Validate signature length (Secp256r1 signature should be 64 bytes)
                require!(self.signature.len() == 64, LazorKitError::InvalidSignature);

                // Validate client data and authenticator data are not empty
                require!(
                    !self.client_data_json_raw.is_empty(),
                    LazorKitError::InvalidInstructionData
                );
                require!(
                    !self.authenticator_data_raw.is_empty(),
                    LazorKitError::InvalidInstructionData
                );

                // Validate verify instruction index
                require!(
                    self.verify_instruction_index < 255,
                    LazorKitError::InvalidInstructionData
                );

                // Validate vault index
                require!(self.vault_index < 32, LazorKitError::InvalidVaultIndex);

                Ok(())
            }
        }
    };
}

impl Args for CreateSessionArgs {
    fn validate(&self) -> Result<()> {
        // Common passkey/signature/client/auth checks
        require!(
            self.passkey_pubkey[0] == 0x02 || self.passkey_pubkey[0] == 0x03,
            LazorKitError::InvalidPasskeyFormat
        );
        require!(self.signature.len() == 64, LazorKitError::InvalidSignature);
        require!(
            !self.client_data_json_raw.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        require!(
            !self.authenticator_data_raw.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        require!(
            self.verify_instruction_index < 255,
            LazorKitError::InvalidInstructionData
        );
        // Split index bounds check left to runtime with account len; ensure policy_data present
        require!(
            !self.policy_data.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        // Validate expires_at within 30s window of now
        let now = Clock::get()?.unix_timestamp;
        require!(
            self.expires_at >= now
                && self.expires_at <= now + crate::security::MAX_SESSION_TTL_SECONDS,
            LazorKitError::TransactionTooOld
        );
        // Validate vault index
        require!(self.vault_index < 32, LazorKitError::InvalidVaultIndex);
        Ok(())
    }
}

// Only ExecuteTransactionArgs has vault_index, so we need separate validation
impl Args for ExecuteTransactionArgs {
    fn validate(&self) -> Result<()> {
        // Validate passkey format
        require!(
            self.passkey_pubkey[0] == 0x02 || self.passkey_pubkey[0] == 0x03,
            LazorKitError::InvalidPasskeyFormat
        );

        // Validate signature length (Secp256r1 signature should be 64 bytes)
        require!(self.signature.len() == 64, LazorKitError::InvalidSignature);

        // Validate client data and authenticator data are not empty
        require!(
            !self.client_data_json_raw.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        require!(
            !self.authenticator_data_raw.is_empty(),
            LazorKitError::InvalidInstructionData
        );

        // Validate verify instruction index
        require!(
            self.verify_instruction_index < 255,
            LazorKitError::InvalidInstructionData
        );

        // Validate vault index
        require!(self.vault_index < 32, LazorKitError::InvalidVaultIndex);

        Ok(())
    }
}

impl_args_validate!(UpdatePolicyArgs);
impl_args_validate!(InvokePolicyArgs);
