use crate::error::LazorKitError;
use anchor_lang::prelude::*;

pub trait Args {
    fn validate(&self) -> Result<()>;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteTxnArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub rule_data: Vec<u8>,
    pub cpi_data: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ChangeRuleArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub old_rule_program: Pubkey,
    pub destroy_rule_data: Vec<u8>,
    pub new_rule_program: Pubkey,
    pub init_rule_data: Vec<u8>,
    pub create_new_authenticator: Option<[u8; 33]>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CallRuleArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub rule_program: Pubkey,
    pub rule_data: Vec<u8>,
    pub create_new_authenticator: Option<[u8; 33]>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CommitArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub rule_data: Vec<u8>,
    pub cpi_program: Pubkey,
    pub expires_at: i64,
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

                Ok(())
            }
        }
    };
}

impl Args for CommitArgs {
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
        // Split index bounds check left to runtime with account len; ensure rule_data present
        require!(
            !self.rule_data.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        Ok(())
    }
}

impl_args_validate!(ExecuteTxnArgs);
impl_args_validate!(ChangeRuleArgs);
impl_args_validate!(CallRuleArgs);
