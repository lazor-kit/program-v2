use anchor_lang::prelude::*;

pub const MAX_TIMESTAMP_DRIFT_SECONDS: i64 = 30;

pub trait Message {
    fn verify(challenge_bytes: Vec<u8>, last_nonce: u64) -> Result<()>;
}

#[derive(Default, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct ExecuteMessage {
    pub nonce: u64,
    pub current_timestamp: i64,
    pub policy_data_hash: [u8; 32],
    pub policy_accounts_hash: [u8; 32],
    pub cpi_data_hash: [u8; 32],
    pub cpi_accounts_hash: [u8; 32],
}

#[derive(Default, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct ExecueSessionMessage {
    pub nonce: u64,
    pub current_timestamp: i64,
    pub policy_data_hash: [u8; 32],
    pub policy_accounts_hash: [u8; 32],
    pub cpi_data_hash: [u8; 32],
    pub cpi_accounts_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Default, Clone)]
pub struct InvokePolicyMessage {
    pub nonce: u64,
    pub current_timestamp: i64,
    pub policy_data_hash: [u8; 32],
    pub policy_accounts_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Default, Clone)]
pub struct UpdatePolicyMessage {
    pub nonce: u64,
    pub current_timestamp: i64,
    pub old_policy_data_hash: [u8; 32],
    pub old_policy_accounts_hash: [u8; 32],
    pub new_policy_data_hash: [u8; 32],
    pub new_policy_accounts_hash: [u8; 32],
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
impl_message_verify!(ExecueSessionMessage);
impl_message_verify!(InvokePolicyMessage);
impl_message_verify!(UpdatePolicyMessage);
