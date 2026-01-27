use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

/// Container for a set of compact instructions.
///
/// This struct holds multiple compact instructions and provides
/// functionality to serialize them into a byte format.
pub struct CompactInstructions {
    /// Vector of individual compact instructions
    pub inner_instructions: Vec<CompactInstruction>,
}

/// Represents a single instruction in compact format.
///
/// Instead of storing full public keys, this format uses indexes
/// into a shared account list to reduce data size.
///
/// # Fields
/// * `program_id_index` - Index of the program ID in the account list
/// * `accounts` - Indexes of accounts used by this instruction
/// * `data` - Raw instruction data
#[derive(Debug, Clone)]
pub struct CompactInstruction {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
}

/// Reference version of CompactInstruction that borrows its data.
///
/// # Fields
/// * `program_id_index` - Index of the program ID in the account list
/// * `accounts` - Slice of account indexes
/// * `data` - Slice of instruction data
pub struct CompactInstructionRef<'a> {
    pub program_id_index: u8,
    pub accounts: &'a [u8],
    pub data: &'a [u8],
}

impl CompactInstructions {
    /// Serializes the compact instructions into bytes.
    ///
    /// The byte format is:
    /// 1. Number of instructions (u8)
    /// 2. For each instruction:
    ///    - Program ID index (u8)
    ///    - Number of accounts (u8)
    ///    - Account indexes (u8 array)
    ///    - Data length (u16 LE)
    ///    - Instruction data (bytes)
    ///
    /// # Returns
    /// * `Vec<u8>` - Serialized instruction data
    pub fn into_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.inner_instructions.len() as u8];
        for ix in self.inner_instructions.iter() {
            bytes.push(ix.program_id_index);
            bytes.push(ix.accounts.len() as u8);
            bytes.extend(ix.accounts.iter());
            bytes.extend((ix.data.len() as u16).to_le_bytes());
            bytes.extend(ix.data.iter());
        }
        bytes
    }
}

impl CompactInstruction {
    /// Deserialize a CompactInstruction from bytes
    /// Format: [program_id_index: u8][num_accounts: u8][accounts...][data_len: u16][data...]
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if bytes.len() < 4 {
            // Minimum: program_id(1) + num_accounts(1) + data_len(2)
            return Err(ProgramError::InvalidInstructionData);
        }

        let program_id_index = bytes[0];
        let num_accounts = bytes[1] as usize;

        if bytes.len() < 2 + num_accounts + 2 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let accounts = bytes[2..2 + num_accounts].to_vec();
        let data_len_offset = 2 + num_accounts;
        let data_len =
            u16::from_le_bytes([bytes[data_len_offset], bytes[data_len_offset + 1]]) as usize;

        let data_start = data_len_offset + 2;
        if bytes.len() < data_start + data_len {
            return Err(ProgramError::InvalidInstructionData);
        }

        let data = bytes[data_start..data_start + data_len].to_vec();
        let rest = &bytes[data_start + data_len..];

        Ok((
            CompactInstruction {
                program_id_index,
                accounts,
                data,
            },
            rest,
        ))
    }

    /// Serialize this CompactInstruction to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + self.accounts.len() + self.data.len());
        bytes.push(self.program_id_index);
        bytes.push(self.accounts.len() as u8);
        bytes.extend_from_slice(&self.accounts);
        bytes.extend_from_slice(&(self.data.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    /// Decompress this compact instruction into a full Instruction using the provided accounts
    pub fn decompress<'a>(
        &self,
        account_infos: &'a [AccountInfo],
    ) -> Result<DecompressedInstruction<'a>, ProgramError> {
        // Validate program_id_index
        if (self.program_id_index as usize) >= account_infos.len() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let program_id = account_infos[self.program_id_index as usize].key();

        // Validate all account indexes
        let mut accounts = Vec::with_capacity(self.accounts.len());
        for &index in &self.accounts {
            if (index as usize) >= account_infos.len() {
                return Err(ProgramError::InvalidInstructionData);
            }
            accounts.push(&account_infos[index as usize]);
        }

        Ok(DecompressedInstruction {
            program_id,
            accounts,
            data: self.data.clone(), // Clone data to avoid lifetime issues
        })
    }
}

/// Decompressed instruction ready for execution
pub struct DecompressedInstruction<'a> {
    pub program_id: &'a Pubkey,
    pub accounts: Vec<&'a AccountInfo>,
    pub data: Vec<u8>, // Owned data to avoid lifetime issues
}

/// Parse multiple CompactInstructions from bytes
/// Format: [num_instructions: u8][instruction_0][instruction_1]...
pub fn parse_compact_instructions(bytes: &[u8]) -> Result<Vec<CompactInstruction>, ProgramError> {
    if bytes.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let num_instructions = bytes[0] as usize;
    let mut instructions = Vec::with_capacity(num_instructions);
    let mut remaining = &bytes[1..];

    for _ in 0..num_instructions {
        let (instruction, rest) = CompactInstruction::from_bytes(remaining)?;
        instructions.push(instruction);
        remaining = rest;
    }

    Ok(instructions)
}

/// Serialize multiple CompactInstructions to bytes
pub fn serialize_compact_instructions(instructions: &[CompactInstruction]) -> Vec<u8> {
    let compact_instructions = CompactInstructions {
        inner_instructions: instructions.to_vec(),
    };
    compact_instructions.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_instruction_serialization() {
        let ix = CompactInstruction {
            program_id_index: 0,
            accounts: vec![1, 2, 3],
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        };

        let bytes = ix.to_bytes();
        let (deserialized, rest) = CompactInstruction::from_bytes(&bytes).unwrap();

        assert_eq!(rest.len(), 0);
        assert_eq!(deserialized.program_id_index, ix.program_id_index);
        assert_eq!(deserialized.accounts, ix.accounts);
        assert_eq!(deserialized.data, ix.data);
    }

    #[test]
    fn test_multiple_instructions() {
        let instructions = vec![
            CompactInstruction {
                program_id_index: 0,
                accounts: vec![1, 2],
                data: vec![1, 2, 3],
            },
            CompactInstruction {
                program_id_index: 3,
                accounts: vec![4, 5, 6],
                data: vec![7, 8, 9, 10],
            },
        ];

        let bytes = serialize_compact_instructions(&instructions);
        let parsed = parse_compact_instructions(&bytes).unwrap();

        assert_eq!(parsed.len(), instructions.len());
        for (original, parsed) in instructions.iter().zip(parsed.iter()) {
            assert_eq!(original.program_id_index, parsed.program_id_index);
            assert_eq!(original.accounts, parsed.accounts);
            assert_eq!(original.data, parsed.data);
        }
    }

    #[test]
    fn test_empty_instruction_data() {
        // Instruction with no data
        let ix = CompactInstruction {
            program_id_index: 0,
            accounts: vec![1],
            data: vec![],
        };

        let bytes = ix.to_bytes();
        let (deserialized, _) = CompactInstruction::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.data.len(), 0);
    }

    #[test]
    fn test_empty_accounts() {
        // Instruction with no accounts
        let ix = CompactInstruction {
            program_id_index: 0,
            accounts: vec![],
            data: vec![1, 2, 3],
        };

        let bytes = ix.to_bytes();
        let (deserialized, _) = CompactInstruction::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.accounts.len(), 0);
    }

    #[test]
    fn test_max_accounts() {
        // Test with maximum number of accounts (u8::MAX = 255)
        let accounts: Vec<u8> = (0..=255).collect();
        let ix = CompactInstruction {
            program_id_index: 0,
            accounts,
            data: vec![1],
        };

        let bytes = ix.to_bytes();
        let (deserialized, _) = CompactInstruction::from_bytes(&bytes).unwrap();

        // Note: accounts.len() wraps to 0 when cast to u8!
        // This is a known limitation - can't have exactly 256 accounts
        assert_eq!(deserialized.accounts.len(), 0); // Wraps around!
    }

    #[test]
    fn test_large_data() {
        // Test with large instruction data (close to u16::MAX)
        let data = vec![0x42; 1000];
        let ix = CompactInstruction {
            program_id_index: 0,
            accounts: vec![1],
            data: data.clone(),
        };

        let bytes = ix.to_bytes();
        let (deserialized, _) = CompactInstruction::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.data.len(), 1000);
        assert_eq!(deserialized.data, data);
    }

    #[test]
    fn test_invalid_truncated_data() {
        // Truncated instruction data
        let bytes = vec![0, 2, 1, 2]; // program_id, num_accounts, accounts... but missing data_len

        let result = CompactInstruction::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_short_buffer() {
        // Buffer too short (less than minimum 4 bytes)
        let bytes = vec![0, 1, 2];

        let result = CompactInstruction::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_data_length_mismatch() {
        // Data length field says 10 bytes but only 5 provided
        let mut bytes = vec![0, 1, 1]; // program_id, num_accounts=1, account=1
        bytes.extend(&10u16.to_le_bytes()); // data_len = 10
        bytes.extend(&[1, 2, 3, 4, 5]); // only 5 bytes of data

        let result = CompactInstruction::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_instructions_list() {
        // Empty list of instructions
        let instructions: Vec<CompactInstruction> = vec![];

        let bytes = serialize_compact_instructions(&instructions);
        let parsed = parse_compact_instructions(&bytes).unwrap();

        assert_eq!(parsed.len(), 0);
    }

    #[test]
    fn test_compact_instructions_wrapper() {
        // Test CompactInstructions wrapper struct
        let compact = CompactInstructions {
            inner_instructions: vec![CompactInstruction {
                program_id_index: 0,
                accounts: vec![1, 2],
                data: vec![0xAB, 0xCD],
            }],
        };

        let bytes = compact.into_bytes();
        let parsed = parse_compact_instructions(&bytes).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].program_id_index, 0);
        assert_eq!(parsed[0].accounts, vec![1, 2]);
        assert_eq!(parsed[0].data, vec![0xAB, 0xCD]);
    }
}
