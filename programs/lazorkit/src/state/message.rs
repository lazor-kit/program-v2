use anchor_lang::prelude::*;

/// Maximum allowed timestamp drift in seconds for message validation
pub const MAX_TIMESTAMP_DRIFT_SECONDS: i64 = 30;

/// Trait for message validation and verification
///
/// All message types must implement this trait to ensure proper
/// timestamp and nonce validation for security and replay attack prevention.
pub trait Message {
    fn verify(challenge_bytes: Vec<u8>, last_nonce: u64) -> Result<()>;
}

/// Message structure for direct transaction execution
///
/// Contains all necessary hashes and metadata required to execute a transaction
/// with policy validation, including nonce and timestamp for security.
#[derive(Default, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct ExecuteMessage {
    /// Nonce to prevent replay attacks
    pub nonce: u64,
    /// Timestamp for message freshness validation
    pub current_timestamp: i64,
    /// Hash of the policy program instruction data
    pub policy_data_hash: [u8; 32],
    /// Hash of the policy program accounts
    pub policy_accounts_hash: [u8; 32],
    /// Hash of the CPI instruction data
    pub cpi_data_hash: [u8; 32],
    /// Hash of the CPI accounts
    pub cpi_accounts_hash: [u8; 32],
}

/// Message structure for creating chunk buffer
///
/// Used for creating chunk buffers when transactions are too large and need
/// to be split into smaller, manageable pieces for processing.
#[derive(Default, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct CreateChunkMessage {
    /// Nonce to prevent replay attacks
    pub nonce: u64,
    /// Timestamp for message freshness validation
    pub current_timestamp: i64,
    /// Hash of the policy program instruction data
    pub policy_data_hash: [u8; 32],
    /// Hash of the policy program accounts
    pub policy_accounts_hash: [u8; 32],
    /// Hash of all CPI instruction data (multiple instructions)
    pub cpi_data_hash: [u8; 32],
    /// Hash of all CPI accounts (multiple instructions)
    pub cpi_accounts_hash: [u8; 32],
    /// Expiration timestamp for the chunk buffer
    pub expires_at: i64,
}

/// Message structure for policy program invocation
///
/// This message is used when invoking policy program methods
/// without executing external transactions.
#[derive(AnchorSerialize, AnchorDeserialize, Debug, Default, Clone)]
pub struct CallPolicyMessage {
    /// Nonce to prevent replay attacks
    pub nonce: u64,
    /// Timestamp for message freshness validation
    pub current_timestamp: i64,
    /// Hash of the policy program instruction data
    pub policy_data_hash: [u8; 32],
    /// Hash of the policy program accounts
    pub policy_accounts_hash: [u8; 32],
}

/// Message structure for wallet policy updates
///
/// This message contains hashes for both old and new policy data
/// to ensure secure policy program transitions.
#[derive(AnchorSerialize, AnchorDeserialize, Debug, Default, Clone)]
pub struct ChangePolicyMessage {
    /// Nonce to prevent replay attacks
    pub nonce: u64,
    /// Timestamp for message freshness validation
    pub current_timestamp: i64,
    /// Hash of the old policy program instruction data
    pub old_policy_data_hash: [u8; 32],
    /// Hash of the old policy program accounts
    pub old_policy_accounts_hash: [u8; 32],
    /// Hash of the new policy program instruction data
    pub new_policy_data_hash: [u8; 32],
    /// Hash of the new policy program accounts
    pub new_policy_accounts_hash: [u8; 32],
}

/// Message structure for ephemeral execution authorization
///
/// This message is used to authorize temporary execution keys that can
/// execute transactions on behalf of the smart wallet without direct passkey authentication.
#[derive(AnchorSerialize, AnchorDeserialize, Debug, Default, Clone)]
pub struct GrantPermissionMessage {
    /// Nonce to prevent replay attacks
    pub nonce: u64,
    /// Timestamp for message freshness validation
    pub current_timestamp: i64,
    /// The ephemeral public key being authorized
    pub ephemeral_key: Pubkey,
    /// Expiration timestamp for the authorization
    pub expires_at: i64,
    /// Hash of all instruction data to be authorized
    pub data_hash: [u8; 32],
    /// Hash of all accounts involved in the authorized transactions
    pub accounts_hash: [u8; 32],
}

macro_rules! impl_message_verify {
    ($t:ty) => {
        impl Message for $t {
            fn verify(challenge_bytes: Vec<u8>, last_nonce: u64) -> Result<()> {
                let hdr: $t = AnchorDeserialize::deserialize(&mut &challenge_bytes[..])
                    .map_err(|_| crate::error::LazorKitError::ChallengeDeserializationError)?;
                let now = Clock::get()?.unix_timestamp;
                if hdr.current_timestamp < now.saturating_sub(MAX_TIMESTAMP_DRIFT_SECONDS) {
                    return Err(crate::error::LazorKitError::TimestampTooOld.into());
                }
                if hdr.current_timestamp > now.saturating_add(MAX_TIMESTAMP_DRIFT_SECONDS) {
                    return Err(crate::error::LazorKitError::TimestampTooNew.into());
                }
                require!(
                    hdr.nonce == last_nonce,
                    crate::error::LazorKitError::NonceMismatch
                );
                Ok(())
            }
        }
    };
}

impl_message_verify!(ExecuteMessage);
impl_message_verify!(CreateChunkMessage);
impl_message_verify!(CallPolicyMessage);
impl_message_verify!(ChangePolicyMessage);
impl_message_verify!(GrantPermissionMessage);
