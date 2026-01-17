//! Policy data format and parsing utilities.
//!
//! This module defines the standard format for policy data storage in LazorKit state.
//! Each policy is stored sequentially with a header followed by its state blob.

use no_padding::NoPadding;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

use crate::{IntoBytes, Transmutable};

/// Header stored before each policy's state blob in the role buffer.
///
/// Format (40 bytes):
/// - program_id: [u8; 32] - The policy program's public key
/// - data_length: u16 - Size of the state_blob in bytes
/// - _padding: u16 - Explicit padding for alignment  
/// - boundary: u32 - Offset to the next policy (or end of policies)
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, NoPadding)]
pub struct PolicyHeader {
    /// Policy program ID that will verify this policy's state
    pub program_id: [u8; 32],
    /// Length of the state blob following this header
    pub data_length: u16,
    /// Explicit padding for 8-byte alignment
    pub _padding: u16,
    /// Offset to the next policy (from start of policies_data)
    pub boundary: u32,
}

impl PolicyHeader {
    /// Size of the policy header in bytes (40 bytes with explicit padding)
    pub const LEN: usize = 40; // 32 + 2 + 2 + 4 = 40 bytes

    /// Creates a new policy header
    pub fn new(program_id: Pubkey, data_length: u16, boundary: u32) -> Self {
        Self {
            program_id: program_id.as_ref().try_into().unwrap(),
            data_length,
            _padding: 0,
            boundary,
        }
    }

    /// Gets the program ID as a Pubkey
    pub fn program_id(&self) -> Pubkey {
        Pubkey::from(self.program_id)
    }
}

impl Transmutable for PolicyHeader {
    const LEN: usize = Self::LEN;
}

impl IntoBytes for PolicyHeader {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        Ok(unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) })
    }
}

/// View into a policy's data within the policies buffer
#[derive(Debug)]
pub struct PolicyView<'a> {
    /// The policy header
    pub header: &'a PolicyHeader,
    /// The policy's state blob
    pub state_blob: &'a [u8],
    /// Index of this policy in the policies array
    pub index: usize,
    /// Offset in the policies_data buffer where this policy starts
    pub offset: usize,
}

/// Iterator over policies in policies_data
pub struct PolicyIterator<'a> {
    policies_data: &'a [u8],
    cursor: usize,
    index: usize,
}

impl<'a> PolicyIterator<'a> {
    /// Creates a new policy iterator
    pub fn new(policies_data: &'a [u8]) -> Self {
        Self {
            policies_data,
            cursor: 0,
            index: 0,
        }
    }
}

impl<'a> Iterator for PolicyIterator<'a> {
    type Item = Result<PolicyView<'a>, ProgramError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.policies_data.len() {
            return None;
        }

        // Check if we have enough data for header
        if self.cursor + PolicyHeader::LEN > self.policies_data.len() {
            return Some(Err(ProgramError::InvalidAccountData));
        }

        // Parse header
        let header_bytes = &self.policies_data[self.cursor..self.cursor + PolicyHeader::LEN];
        let header = unsafe {
            match PolicyHeader::load_unchecked(header_bytes) {
                Ok(h) => h,
                Err(e) => return Some(Err(e)),
            }
        };

        // Calculate state blob range
        let blob_start = self.cursor + PolicyHeader::LEN;
        let blob_end = blob_start + header.data_length as usize;

        // Check if we have enough data for blob
        if blob_end > self.policies_data.len() {
            return Some(Err(ProgramError::InvalidAccountData));
        }

        let state_blob = &self.policies_data[blob_start..blob_end];

        let policy_view = PolicyView {
            header,
            state_blob,
            index: self.index,
            offset: self.cursor,
        };

        // Move cursor to next policy
        self.cursor = header.boundary as usize;
        self.index += 1;

        Some(Ok(policy_view))
    }
}

/// Parses policies_data into individual policies
pub fn parse_policies(policies_data: &[u8]) -> PolicyIterator<'_> {
    PolicyIterator::new(policies_data)
}

/// Counts the number of policies in policies_data
pub fn count_policies(policies_data: &[u8]) -> Result<u16, ProgramError> {
    let mut count = 0u16;
    for result in parse_policies(policies_data) {
        result?; // Validate each policy
        count = count.saturating_add(1);
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_header_size() {
        assert_eq!(PolicyHeader::LEN, 40);
        assert_eq!(core::mem::size_of::<PolicyHeader>(), 40);
    }

    #[test]
    fn test_parse_empty() {
        let policies_data = [];
        let policies: Vec<_> = parse_policies(&policies_data).collect();
        assert_eq!(policies.len(), 0);
    }

    #[test]
    fn test_parse_single_policy() {
        let program_id = Pubkey::from([1u8; 32]);
        let state_data = [42u8; 8];

        let header = PolicyHeader::new(program_id, 8, 40 + 8);
        let header_bytes = header.into_bytes().unwrap();

        let mut policies_data = Vec::new();
        policies_data.extend_from_slice(header_bytes);
        policies_data.extend_from_slice(&state_data);

        let policies: Vec<_> = parse_policies(&policies_data)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].header.program_id(), program_id);
        assert_eq!(policies[0].state_blob, &state_data);
        assert_eq!(policies[0].index, 0);
    }

    #[test]
    fn test_count_policies() {
        let program_id = Pubkey::from([1u8; 32]);
        let state_data = [42u8; 8];

        let header = PolicyHeader::new(program_id, 8, 40 + 8);
        let header_bytes = header.into_bytes().unwrap();

        let mut policies_data = Vec::new();
        policies_data.extend_from_slice(header_bytes);
        policies_data.extend_from_slice(&state_data);

        let count = count_policies(&policies_data).unwrap();
        assert_eq!(count, 1);
    }
}
