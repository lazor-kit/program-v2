use crate::error::AuthError;
use pinocchio::program_error::ProgramError;

/// Secp256r1 program ID
pub const SECP256R1_PROGRAM_ID: [u8; 32] = [
    6, 146, 13, 236, 47, 234, 113, 181, 183, 35, 129, 77, 116, 45, 169, 3, 28, 131, 231, 95, 219,
    121, 93, 86, 142, 117, 71, 128, 32, 0, 0, 0,
]; // "Secp256r1SigVerify1111111111111111111111111"

/// Constants from the secp256r1 program
pub const COMPRESSED_PUBKEY_SERIALIZED_SIZE: usize = 33; // Stored 33-byte key (0x02/0x03 + X)
pub const PRECOMPILE_PUBKEY_SERIALIZED_SIZE: usize = 33; // Precompile also uses 33-byte Compressed key!
pub const SIGNATURE_SERIALIZED_SIZE: usize = 64;
pub const SIGNATURE_OFFSETS_SERIALIZED_SIZE: usize = 14;
pub const SIGNATURE_OFFSETS_START: usize = 2; // Matches native precompile [num_sigs(1)][padding(1)]
pub const DATA_START: usize = SIGNATURE_OFFSETS_SERIALIZED_SIZE + SIGNATURE_OFFSETS_START;

pub const SIGNATURE_DATA_OFFSET: usize = DATA_START;
pub const PUBKEY_DATA_OFFSET: usize = DATA_START + SIGNATURE_SERIALIZED_SIZE; // 16 + 64 = 80
                                                                              // Precompile uses the 64-byte RAW key, so the message offset must account for 64 bytes
pub const MESSAGE_DATA_OFFSET: usize = PUBKEY_DATA_OFFSET + PRECOMPILE_PUBKEY_SERIALIZED_SIZE + 1; // 80 + 33 + 1 = 114 (Padding for alignment)
pub const MESSAGE_DATA_SIZE: usize = 32;

/// Secp256r1 signature offsets structure (matches solana-secp256r1-program)
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Secp256r1SignatureOffsets {
    /// Offset to compact secp256r1 signature of 64 bytes
    pub signature_offset: u16,
    /// Instruction index where the signature can be found
    pub signature_instruction_index: u16,
    /// Offset to compressed public key of 33 bytes
    pub public_key_offset: u16,
    /// Instruction index where the public key can be found
    pub public_key_instruction_index: u16,
    /// Offset to the start of message data
    pub message_data_offset: u16,
    /// Size of message data in bytes
    pub message_data_size: u16,
    /// Instruction index where the message data can be found
    pub message_instruction_index: u16,
}

impl Secp256r1SignatureOffsets {
    /// Deserialize from bytes (14 bytes in little-endian format)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProgramError> {
        if bytes.len() != SIGNATURE_OFFSETS_SERIALIZED_SIZE {
            return Err(AuthError::InvalidInstruction.into());
        }

        Ok(Self {
            signature_offset: u16::from_le_bytes([bytes[0], bytes[1]]),
            signature_instruction_index: u16::from_le_bytes([bytes[2], bytes[3]]),
            public_key_offset: u16::from_le_bytes([bytes[4], bytes[5]]),
            public_key_instruction_index: u16::from_le_bytes([bytes[6], bytes[7]]),
            message_data_offset: u16::from_le_bytes([bytes[8], bytes[9]]),
            message_data_size: u16::from_le_bytes([bytes[10], bytes[11]]),
            message_instruction_index: u16::from_le_bytes([bytes[12], bytes[13]]),
        })
    }
}

/// Verify the secp256r1 instruction data contains the expected signature and
/// public key. Also validates that the secp256r1 precompile offsets point to
/// the expected locations, ensuring proper data alignment.
///
/// The expected precompile message is passed as TWO slices — the
/// authenticator_data and the clientDataJSON hash — which are concatenated
/// by the on-chain secp256r1 precompile as its signed message. Accepting
/// two slices here lets the caller skip a Vec allocation for the concat.
pub fn verify_secp256r1_instruction_data(
    instruction_data: &[u8],
    expected_pubkey: &[u8; 33],
    auth_data: &[u8],
    client_data_hash: &[u8; 32],
) -> Result<(), ProgramError> {
    // Minimum check: must have at least the header and offsets
    if instruction_data.len() < DATA_START {
        return Err(AuthError::InvalidInstruction.into());
    }
    let num_signatures = instruction_data[0] as usize;
    if num_signatures == 0 || num_signatures > 1 {
        return Err(AuthError::InvalidInstruction.into());
    }

    if instruction_data.len() < MESSAGE_DATA_OFFSET + MESSAGE_DATA_SIZE {
        return Err(AuthError::InvalidInstruction.into());
    }

    // Parse the Secp256r1SignatureOffsets structure
    let offsets = Secp256r1SignatureOffsets::from_bytes(
        &instruction_data
            [SIGNATURE_OFFSETS_START..SIGNATURE_OFFSETS_START + SIGNATURE_OFFSETS_SERIALIZED_SIZE],
    )?;

    // Validate that all offsets point to the current instruction (0xFFFF only).
    // Rejecting index 0 prevents referencing a different instruction's data,
    // which could allow signature/pubkey/message substitution attacks.
    if offsets.signature_instruction_index != 0xFFFF {
        return Err(AuthError::InvalidInstruction.into());
    }
    if offsets.public_key_instruction_index != 0xFFFF {
        return Err(AuthError::InvalidInstruction.into());
    }
    if offsets.message_instruction_index != 0xFFFF {
        return Err(AuthError::InvalidInstruction.into());
    }

    // Validate that ALL offsets match the expected fixed locations.
    // This ensures the precompile is verifying exactly the data we're checking.
    if offsets.signature_offset as usize != SIGNATURE_DATA_OFFSET {
        return Err(AuthError::InvalidInstruction.into());
    }
    if offsets.public_key_offset as usize != PUBKEY_DATA_OFFSET {
        return Err(AuthError::InvalidInstruction.into());
    }
    if offsets.message_data_offset as usize != MESSAGE_DATA_OFFSET {
        return Err(AuthError::InvalidInstruction.into());
    }
    let expected_msg_len = auth_data.len() + client_data_hash.len();
    if offsets.message_data_size as usize != expected_msg_len {
        return Err(AuthError::InvalidInstruction.into());
    }

    // Dynamic length check: instruction must contain the full message
    if instruction_data.len() < MESSAGE_DATA_OFFSET + expected_msg_len {
        return Err(AuthError::InvalidInstruction.into());
    }

    let pubkey_data = &instruction_data
        [PUBKEY_DATA_OFFSET..PUBKEY_DATA_OFFSET + COMPRESSED_PUBKEY_SERIALIZED_SIZE];
    if pubkey_data != expected_pubkey {
        return Err(AuthError::InvalidPubkey.into());
    }

    // Compare the precompile's message area against the two caller-supplied
    // slices piecewise — no concat, no allocation.
    let msg_auth = &instruction_data[MESSAGE_DATA_OFFSET..MESSAGE_DATA_OFFSET + auth_data.len()];
    if msg_auth != auth_data {
        return Err(AuthError::InvalidMessageHash.into());
    }
    let hash_start = MESSAGE_DATA_OFFSET + auth_data.len();
    let msg_hash = &instruction_data[hash_start..hash_start + client_data_hash.len()];
    if msg_hash != client_data_hash {
        return Err(AuthError::InvalidMessageHash.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build valid secp256r1 precompile instruction data with the standard layout.
    fn build_precompile_ix_data(
        pubkey: &[u8; 33],
        signature: &[u8; 64],
        message: &[u8],
    ) -> Vec<u8> {
        let total_len = DATA_START + 64 + 33 + 1 + message.len();
        let mut data = vec![0u8; total_len];

        // Header
        data[0] = 1; // num_signatures
        data[1] = 0; // padding

        // Offsets (little-endian)
        data[2..4].copy_from_slice(&(SIGNATURE_DATA_OFFSET as u16).to_le_bytes());
        data[4..6].copy_from_slice(&0xFFFFu16.to_le_bytes()); // sig ix index
        data[6..8].copy_from_slice(&(PUBKEY_DATA_OFFSET as u16).to_le_bytes());
        data[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes()); // pubkey ix index
        data[10..12].copy_from_slice(&(MESSAGE_DATA_OFFSET as u16).to_le_bytes());
        data[12..14].copy_from_slice(&(message.len() as u16).to_le_bytes()); // msg size
        data[14..16].copy_from_slice(&0xFFFFu16.to_le_bytes()); // msg ix index

        // Data
        data[SIGNATURE_DATA_OFFSET..SIGNATURE_DATA_OFFSET + 64].copy_from_slice(signature);
        data[PUBKEY_DATA_OFFSET..PUBKEY_DATA_OFFSET + 33].copy_from_slice(pubkey);
        // Byte at offset 113 is alignment padding (zero)
        data[MESSAGE_DATA_OFFSET..MESSAGE_DATA_OFFSET + message.len()]
            .copy_from_slice(message);

        data
    }

    #[test]
    fn test_verify_valid_instruction_data() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_ok());
    }

    #[test]
    fn test_verify_variable_length_message() {
        // Mode 1 messages are authenticatorData(37+) + clientDataJsonHash(32) = 69+ bytes.
        // We split into the two halves exactly like the caller does post-refactor.
        let pubkey = [0x03; 33];
        let signature = [0xCD; 64];
        let message = [0x22; 69];

        let ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        let auth_data: &[u8] = &message[..37];
        let client_data_hash: &[u8; 32] = &message[37..].try_into().unwrap();
        assert!(
            verify_secp256r1_instruction_data(&ix_data, &pubkey, auth_data, client_data_hash)
                .is_ok()
        );
    }

    #[test]
    fn test_verify_rejects_wrong_pubkey() {
        let pubkey = [0x02; 33];
        let wrong_pubkey = [0x03; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        let err =
            verify_secp256r1_instruction_data(&ix_data, &wrong_pubkey, &[], &message).unwrap_err();
        assert_eq!(err, AuthError::InvalidPubkey.into());
    }

    #[test]
    fn test_verify_rejects_wrong_message() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];
        let wrong_message = [0x22; 32];

        let ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        let err =
            verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &wrong_message).unwrap_err();
        assert_eq!(err, AuthError::InvalidMessageHash.into());
    }

    #[test]
    fn test_verify_rejects_zero_signatures() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        ix_data[0] = 0; // zero signatures
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_multiple_signatures() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        ix_data[0] = 2; // two signatures
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_cross_instruction_sig_index() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        // Set signature_instruction_index to 0 instead of 0xFFFF
        ix_data[4..6].copy_from_slice(&0u16.to_le_bytes());
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_cross_instruction_pubkey_index() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        // Set public_key_instruction_index to 1 instead of 0xFFFF
        ix_data[8..10].copy_from_slice(&1u16.to_le_bytes());
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_cross_instruction_msg_index() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        // Set message_instruction_index to 0 instead of 0xFFFF
        ix_data[14..16].copy_from_slice(&0u16.to_le_bytes());
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_wrong_pubkey_offset() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        // Tamper pubkey_offset to point elsewhere
        ix_data[6..8].copy_from_slice(&200u16.to_le_bytes());
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_wrong_message_offset() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        // Tamper message_data_offset
        ix_data[10..12].copy_from_slice(&50u16.to_le_bytes());
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_wrong_signature_offset() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        // Tamper signature_offset
        ix_data[2..4].copy_from_slice(&100u16.to_le_bytes());
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_message_size_mismatch() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        // Set message_data_size to wrong value
        ix_data[12..14].copy_from_slice(&64u16.to_le_bytes());
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_too_short_data() {
        let pubkey = [0x02; 33];
        let message = [0x11; 32];

        // Only 2 bytes — way too short
        assert!(verify_secp256r1_instruction_data(&[0x01, 0x00], &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_verify_rejects_truncated_message_area() {
        let pubkey = [0x02; 33];
        let signature = [0xAB; 64];
        let message = [0x11; 32];

        let mut ix_data = build_precompile_ix_data(&pubkey, &signature, &message);
        // Truncate — remove last 10 bytes so message area is incomplete
        ix_data.truncate(ix_data.len() - 10);
        assert!(verify_secp256r1_instruction_data(&ix_data, &pubkey, &[], &message).is_err());
    }

    #[test]
    fn test_offsets_constants_are_consistent() {
        assert_eq!(DATA_START, 16); // 2 header + 14 offsets
        assert_eq!(SIGNATURE_DATA_OFFSET, 16);
        assert_eq!(PUBKEY_DATA_OFFSET, 16 + 64); // 80
        assert_eq!(MESSAGE_DATA_OFFSET, 80 + 33 + 1); // 114
    }
}
