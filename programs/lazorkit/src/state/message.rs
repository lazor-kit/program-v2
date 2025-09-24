use anchor_lang::prelude::*;

/// Maximum allowed timestamp drift in seconds for message validation
pub const MAX_TIMESTAMP_DRIFT_SECONDS: i64 = 30;

/// Trait for message validation and verification
///
/// All message types must implement this trait to ensure proper
/// hash verification for security and replay attack prevention.
pub trait Message {
    /// Verify the message hash against the provided challenge bytes
    fn verify_hash(challenge_bytes: Vec<u8>, expected_hash: [u8; 32]) -> Result<()>;
}

/// Simplified message structure - all messages are now just 32-byte hashes
///
/// The message contains only a single hash that represents the entire message data.
/// On-chain verification will hash the actual data and compare with this hash.
#[derive(Default, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct SimpleMessage {
    /// Single hash representing the entire message data
    pub data_hash: [u8; 32],
}

// All message types now use SimpleMessage - no need for separate structures

impl Message for SimpleMessage {
    fn verify_hash(challenge_bytes: Vec<u8>, expected_hash: [u8; 32]) -> Result<()> {
        let message: SimpleMessage = AnchorDeserialize::deserialize(&mut &challenge_bytes[..])
            .map_err(|_| crate::error::LazorKitError::ChallengeDeserializationError)?;

        require!(
            message.data_hash == expected_hash,
            crate::error::LazorKitError::HashMismatch
        );

        Ok(())
    }
}
