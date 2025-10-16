use crate::validate_webauthn_args;
use crate::{constants::PASSKEY_PUBLIC_KEY_SIZE, error::LazorKitError};
use anchor_lang::prelude::*;

/// Trait for argument validation
///
/// All instruction argument structs must implement this trait to ensure
/// proper validation of input parameters before processing.
pub trait Args {
    fn validate(&self) -> Result<()>;
}

/// Arguments for creating a new smart wallet
///
/// Contains all necessary parameters for initializing a new smart wallet
/// with WebAuthn passkey authentication and policy program configuration.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateSmartWalletArgs {
    /// Public key of the WebAuthn passkey for authentication
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// Unique credential ID from WebAuthn registration
    pub credential_hash: [u8; 32],
    /// Policy program initialization data
    pub init_policy_data: Vec<u8>,
    /// Random wallet ID provided by client for uniqueness
    pub wallet_id: u64,
    /// Initial SOL amount to transfer to the wallet
    pub amount: u64,
    /// Optional referral address for fee sharing
    pub referral_address: Option<Pubkey>,
}

/// Arguments for executing a transaction through the smart wallet
///
/// Contains WebAuthn authentication data and transaction parameters
/// required for secure transaction execution with policy validation.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteArgs {
    /// Public key of the WebAuthn passkey for authentication
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// WebAuthn signature for transaction authorization
    pub signature: Vec<u8>,
    /// Raw client data JSON from WebAuthn authentication
    pub client_data_json_raw: Vec<u8>,
    /// Raw authenticator data from WebAuthn authentication
    pub authenticator_data_raw: Vec<u8>,
    /// Index of the Secp256r1 verification instruction
    pub verify_instruction_index: u8,
    /// Index for splitting remaining accounts between policy and CPI
    pub split_index: u16,
    /// Policy program instruction data
    pub policy_data: Vec<u8>,
    /// Cross-program invocation instruction data
    pub cpi_data: Vec<u8>,
    /// Random vault index (0-31) calculated off-chain for fee distribution
    pub vault_index: u8,
    /// Unix timestamp for message verification
    pub timestamp: i64,
}

/// Arguments for changing a smart wallet's policy program
///
/// Contains WebAuthn authentication data and policy program parameters
/// required for securely changing the policy program governing a wallet.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ChangePolicyArgs {
    /// Public key of the WebAuthn passkey for authentication
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// WebAuthn signature for transaction authorization
    pub signature: Vec<u8>,
    /// Raw client data JSON from WebAuthn authentication
    pub client_data_json_raw: Vec<u8>,
    /// Raw authenticator data from WebAuthn authentication
    pub authenticator_data_raw: Vec<u8>,
    /// Index of the Secp256r1 verification instruction
    pub verify_instruction_index: u8,
    /// Index for splitting remaining accounts between policy and CPI
    pub split_index: u16,
    /// Data for destroying the old policy program
    pub destroy_policy_data: Vec<u8>,
    /// Data for initializing the new policy program
    pub init_policy_data: Vec<u8>,
    /// Optional new wallet device to add during policy change
    pub new_wallet_device: Option<NewWalletDeviceArgs>,
    /// Random vault index (0-31) calculated off-chain for fee distribution
    pub vault_index: u8,
    /// Unix timestamp for message verification
    pub timestamp: i64,
}

/// Arguments for calling policy program instructions
///
/// Contains WebAuthn authentication data and policy program parameters
/// required for executing policy program instructions like adding/removing devices.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CallPolicyArgs {
    /// Public key of the WebAuthn passkey for authentication
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// WebAuthn signature for transaction authorization
    pub signature: Vec<u8>,
    /// Raw client data JSON from WebAuthn authentication
    pub client_data_json_raw: Vec<u8>,
    /// Raw authenticator data from WebAuthn authentication
    pub authenticator_data_raw: Vec<u8>,
    /// Index of the Secp256r1 verification instruction
    pub verify_instruction_index: u8,
    /// Policy program instruction data
    pub policy_data: Vec<u8>,
    /// Optional new wallet device to add during policy call
    pub new_wallet_device: Option<NewWalletDeviceArgs>,
    /// Random vault index (0-31) calculated off-chain for fee distribution
    pub vault_index: u8,
    /// Unix timestamp for message verification
    pub timestamp: i64,
    /// Whether the smart wallet is the signer
    pub smart_wallet_is_signer: bool,
}

/// Arguments for creating a chunk buffer for large transactions
///
/// Contains WebAuthn authentication data and parameters required for
/// creating chunk buffers when transactions exceed size limits.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateChunkArgs {
    /// Public key of the WebAuthn passkey for authentication
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// WebAuthn signature for transaction authorization
    pub signature: Vec<u8>,
    /// Raw client data JSON from WebAuthn authentication
    pub client_data_json_raw: Vec<u8>,
    /// Raw authenticator data from WebAuthn authentication
    pub authenticator_data_raw: Vec<u8>,
    /// Index of the Secp256r1 verification instruction
    pub verify_instruction_index: u8,
    /// Policy program instruction data
    pub policy_data: Vec<u8>,
    /// Random vault index (0-31) calculated off-chain for fee distribution
    pub vault_index: u8,
    /// Unix timestamp for message verification (must be <= on-chain time + 30s)
    pub timestamp: i64,
    /// Hash of CPI data and accounts (32 bytes)
    pub cpi_hash: [u8; 32],
}

/// Arguments for adding a new wallet device (passkey)
///
/// Contains the necessary data for adding a new WebAuthn passkey
/// to an existing smart wallet for enhanced security and convenience.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct NewWalletDeviceArgs {
    /// Public key of the new WebAuthn passkey
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],

    /// Unique credential ID from WebAuthn registration (max 256 bytes)
    pub credential_hash: [u8; 32],
}

/// Arguments for granting ephemeral permission to a keypair
///
/// Contains WebAuthn authentication data and parameters required for
/// granting time-limited permission to an ephemeral keypair for
/// multiple operations without repeated passkey authentication.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct GrantPermissionArgs {
    /// Public key of the WebAuthn passkey for authentication
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// WebAuthn signature for transaction authorization
    pub signature: Vec<u8>,
    /// Raw client data JSON from WebAuthn authentication
    pub client_data_json_raw: Vec<u8>,
    /// Raw authenticator data from WebAuthn authentication
    pub authenticator_data_raw: Vec<u8>,
    /// Index of the Secp256r1 verification instruction
    pub verify_instruction_index: u8,
    /// Ephemeral public key that will receive permission
    pub ephemeral_public_key: Pubkey,
    /// Unix timestamp when the permission expires
    pub expires_at: i64,
    /// Random vault index (0-31) calculated off-chain for fee distribution
    pub vault_index: u8,
    /// All instruction data to be authorized for execution
    pub instruction_data_list: Vec<Vec<u8>>,
    /// Split indices for accounts (n-1 for n instructions)
    pub split_index: Vec<u8>,
    /// Unix timestamp for message verification
    pub timestamp: i64,
}

impl Args for CreateChunkArgs {
    fn validate(&self) -> Result<()> {
        // Common passkey/signature/client/auth checks
        require!(
            self.passkey_public_key[0] == crate::constants::SECP256R1_COMPRESSED_PUBKEY_PREFIX_EVEN
                || self.passkey_public_key[0]
                    == crate::constants::SECP256R1_COMPRESSED_PUBKEY_PREFIX_ODD,
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
            self.verify_instruction_index <= crate::constants::MAX_VERIFY_INSTRUCTION_INDEX,
            LazorKitError::InvalidInstructionData
        );
        // Split index bounds check left to runtime with account len; ensure policy_data present
        require!(
            !self.policy_data.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        // Validate vault index with enhanced validation
        crate::security::validation::validate_vault_index_enhanced(self.vault_index)?;
        // Validate timestamp using standardized validation
        crate::security::validation::validate_instruction_timestamp(self.timestamp)?;
        Ok(())
    }
}

// Only ExecuteArgs has vault_index, so we need separate validation
impl Args for ExecuteArgs {
    fn validate(&self) -> Result<()> {
        validate_webauthn_args!(self);
        Ok(())
    }
}

macro_rules! impl_args_validate {
    ($t:ty) => {
        impl Args for $t {
            fn validate(&self) -> Result<()> {
                validate_webauthn_args!(self);
                Ok(())
            }
        }
    };
}

impl_args_validate!(ChangePolicyArgs);
impl_args_validate!(CallPolicyArgs);
impl_args_validate!(GrantPermissionArgs);
