//! Policy Registry Entry State

use no_padding::NoPadding;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

use crate::{IntoBytes, Transmutable, TransmutableMut};

/// Registry entry for a verified policy.
///
/// Seeds: ["policy-registry", policy_program_id]
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, NoPadding)]
pub struct PolicyRegistryEntry {
    /// Version (0..8)
    pub version: u64,
    /// Timestamp (8..16)
    pub added_at: i64,
    /// Program ID (16..48)
    pub program_id: [u8; 32],
    /// Is Active (48..49) - Using u8 for explicit size
    pub is_active: u8,
    /// Bump (49..50)
    pub bump: u8,
    /// Padding (50..56)
    pub _padding: [u8; 6],
}

impl PolicyRegistryEntry {
    pub const LEN: usize = 56;
    pub const SEED_PREFIX: &'static [u8] = b"policy-registry";
    pub const VERSION: u64 = 1;

    pub fn new(program_id: Pubkey, bump: u8, current_time: i64) -> Self {
        let mut pid_bytes = [0u8; 32];
        pid_bytes.copy_from_slice(program_id.as_ref());

        Self {
            version: Self::VERSION,
            program_id: pid_bytes,
            is_active: 1, // true
            added_at: current_time,
            bump,
            _padding: [0; 6],
        }
    }
}

impl Transmutable for PolicyRegistryEntry {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for PolicyRegistryEntry {}

impl IntoBytes for PolicyRegistryEntry {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        Ok(unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) })
    }
}
