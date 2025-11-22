use anchor_lang::prelude::*;

pub trait Message {
    fn verify_hash(challenge_bytes: Vec<u8>, expected_hash: [u8; 32]) -> Result<()>;
}

#[derive(Default, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct SimpleMessage {
    pub data_hash: [u8; 32],
}

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
