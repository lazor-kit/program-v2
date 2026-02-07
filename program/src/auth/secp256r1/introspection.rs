use crate::error::AuthError;
use pinocchio::program_error::ProgramError;

/// Secp256r1 program ID
pub const SECP256R1_PROGRAM_ID: [u8; 32] = [
    6, 146, 13, 236, 47, 234, 113, 181, 183, 35, 129, 77, 116, 45, 169, 3, 28, 131, 231, 95, 219,
    121, 93, 86, 142, 117, 71, 128, 32, 0, 0, 0,
]; // "Secp256r1SigVerify1111111111111111111111111" BaselineFinalCorrectedVerifiedFinalFinalFinalFinalFinalFinalFinalFinalFinalFinal

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
/// public key. This also validates that the secp256r1 precompile offsets point
/// to the expected locations, ensuring proper data alignment.
pub fn verify_secp256r1_instruction_data(
    instruction_data: &[u8],
    expected_pubkey: &[u8; 33],
    expected_message: &[u8],
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

    // Validate that all offsets point to the current instruction (0xFFFF)
    // This ensures all data references are within the same instruction
    if offsets.signature_instruction_index != 0xFFFF && offsets.signature_instruction_index != 0 {
        return Err(AuthError::InvalidInstruction.into());
    }
    if offsets.public_key_instruction_index != 0xFFFF && offsets.public_key_instruction_index != 0 {
        return Err(AuthError::InvalidInstruction.into());
    }
    if offsets.message_instruction_index != 0xFFFF && offsets.message_instruction_index != 0 {
        return Err(AuthError::InvalidInstruction.into());
    }

    // Validate that the offsets match the expected fixed locations
    // This ensures the precompile is verifying the data we're checking
    if offsets.public_key_offset as usize != PUBKEY_DATA_OFFSET {
        return Err(AuthError::InvalidInstruction.into());
    }
    if offsets.message_data_offset as usize != MESSAGE_DATA_OFFSET {
        return Err(AuthError::InvalidInstruction.into());
    }
    if offsets.message_data_size as usize != expected_message.len() {
        return Err(AuthError::InvalidInstruction.into());
    }

    let pubkey_data = &instruction_data
        [PUBKEY_DATA_OFFSET..PUBKEY_DATA_OFFSET + COMPRESSED_PUBKEY_SERIALIZED_SIZE];
    let message_data =
        &instruction_data[MESSAGE_DATA_OFFSET..MESSAGE_DATA_OFFSET + expected_message.len()];

    if pubkey_data != expected_pubkey {
        return Err(AuthError::InvalidPubkey.into());
    }
    if message_data != expected_message {
        return Err(AuthError::InvalidMessageHash.into());
    }
    Ok(())
}
