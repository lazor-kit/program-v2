//! LazorKit Plugin Interface
//!
//! This crate defines the standard interface that all LazorKit plugins must implement.
//! It provides the core types and traits for plugin validation and state management.

use no_padding::NoPadding;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

/// Instruction discriminator for Verify
pub const INSTRUCTION_VERIFY: u64 = 0x89723049; // Random magic number or hashed name

/// Context provided to plugins during verification
///
/// This struct is passed as instruction data to the plugin via CPI.
/// The plugin uses `state_offset` to locate and modify its state
/// directly within the LazorKit wallet account (passed as Account 0).
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, NoPadding)]
pub struct VerifyInstruction {
    /// Discriminator to identify the instruction type
    pub discriminator: u64,

    /// Offset to the start of this plugin's state in the wallet account data
    pub state_offset: u32,

    /// The role ID executing this action
    pub role_id: u32,

    /// Current Solana slot
    pub slot: u64,

    /// Amount involved in the operation (e.g. SOL spent)
    pub amount: u64,

    /// Reserved for future use / alignment
    pub _reserved: [u64; 4],
}

impl VerifyInstruction {
    pub const LEN: usize = 8 + 4 + 4 + 8 + 8 + 32; // 64 bytes
}

/// Error codes for plugin operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PluginError {
    /// Verification failed - transaction not allowed
    VerificationFailed = 1000,
    /// Invalid state data format
    InvalidStateData = 1001,
    /// Invalid context data
    InvalidContext = 1002,
}

impl From<PluginError> for ProgramError {
    fn from(e: PluginError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
