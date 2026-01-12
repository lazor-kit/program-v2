//! Plugin entry structure.
//!
//! Plugins are external programs. We only store:
//! - program_id: The plugin program ID
//! - config_account: The plugin's config PDA
//! - enabled: Whether the plugin is enabled
//! - priority: Execution order (0 = highest priority)

use crate::{IntoBytes, Transmutable};
use no_padding::NoPadding;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

/// Plugin entry in the plugin registry.
///
/// Each plugin is an external program that can check permissions for instructions.
/// Plugins are identified by their program_id, not by a type enum.
#[repr(C, align(8))]
#[derive(Debug, PartialEq, Clone, Copy, NoPadding)]
pub struct PluginEntry {
    pub program_id: Pubkey,     // Plugin program ID (32 bytes)
    pub config_account: Pubkey, // Plugin's config PDA (32 bytes)
    pub enabled: u8,            // 1 = enabled, 0 = disabled (1 byte)
    pub priority: u8,           // Execution order (0 = highest priority) (1 byte)
    pub _padding: [u8; 6],      // Padding to align to 8 bytes (6 bytes)
                                // Total: 32 + 32 + 1 + 1 + 6 = 72 bytes (aligned to 8)
}

impl PluginEntry {
    pub const LEN: usize = core::mem::size_of::<Self>();

    /// Create a new PluginEntry
    pub fn new(program_id: Pubkey, config_account: Pubkey, priority: u8, enabled: bool) -> Self {
        Self {
            program_id,
            config_account,
            enabled: if enabled { 1 } else { 0 },
            priority,
            _padding: [0; 6],
        }
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled == 1
    }
}

impl Transmutable for PluginEntry {
    const LEN: usize = Self::LEN;
}

impl IntoBytes for PluginEntry {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        let bytes =
            unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) };
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinocchio::pubkey::Pubkey;

    #[test]
    fn test_plugin_entry_creation() {
        let program_id = Pubkey::default();
        let config_account = Pubkey::default();
        let entry = PluginEntry::new(program_id, config_account, 0, true);

        assert_eq!(entry.program_id, program_id);
        assert_eq!(entry.config_account, config_account);
        assert!(entry.is_enabled());
    }

    #[test]
    fn test_plugin_entry_size() {
        assert_eq!(PluginEntry::LEN, 72); // 32 + 32 + 1 + 1 + 6 = 72
    }

    #[test]
    fn test_plugin_entry_serialization() {
        let program_id = Pubkey::default();
        let config_account = Pubkey::default();
        let entry = PluginEntry::new(program_id, config_account, 5, false);

        let bytes = entry.into_bytes().unwrap();
        assert_eq!(bytes.len(), PluginEntry::LEN);

        // Deserialize
        let loaded = unsafe { PluginEntry::load_unchecked(bytes).unwrap() };
        assert_eq!(*loaded, entry);
    }
}
