//! Plugin entry structure.

use crate::{Transmutable, IntoBytes};
use no_padding::NoPadding;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

/// Plugin type identifier
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    RolePermission = 0,
    SolLimit = 1,
    TokenLimit = 2,
    ProgramWhitelist = 3,
    Custom = 255,
}

/// Plugin entry in the plugin registry.
///
/// Each plugin is an external program that can check permissions for instructions.
#[repr(C, align(8))]
#[derive(Debug, PartialEq, Clone, Copy, NoPadding)]
pub struct PluginEntry {
    pub program_id: Pubkey,        // Plugin program ID (32 bytes)
    pub config_account: Pubkey,     // Plugin's config PDA (32 bytes)
    pub plugin_type: u8,            // PluginType (1 byte)
    pub enabled: u8,                // 1 = enabled, 0 = disabled
    pub priority: u8,               // Execution order (0 = highest priority)
    pub _padding: [u8; 5],          // Padding to align to 8 bytes
}

impl PluginEntry {
    pub const LEN: usize = core::mem::size_of::<Self>();
    
    /// Create a new PluginEntry
    pub fn new(
        program_id: Pubkey,
        config_account: Pubkey,
        plugin_type: PluginType,
        priority: u8,
        enabled: bool,
    ) -> Self {
        Self {
            program_id,
            config_account,
            plugin_type: plugin_type as u8,
            enabled: if enabled { 1 } else { 0 },
            priority,
            _padding: [0; 5],
        }
    }
    
    /// Get plugin type
    pub fn plugin_type(&self) -> PluginType {
        match self.plugin_type {
            0 => PluginType::RolePermission,
            1 => PluginType::SolLimit,
            2 => PluginType::TokenLimit,
            3 => PluginType::ProgramWhitelist,
            _ => PluginType::Custom,
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
        let bytes = unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN)
        };
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
        let entry = PluginEntry::new(
            program_id,
            config_account,
            PluginType::RolePermission,
            0,
            true,
        );
        
        assert_eq!(entry.program_id, program_id);
        assert_eq!(entry.config_account, config_account);
        assert_eq!(entry.plugin_type(), PluginType::RolePermission);
        assert!(entry.is_enabled());
    }
    
    #[test]
    fn test_plugin_entry_size() {
        assert_eq!(PluginEntry::LEN, 72); // 32 + 32 + 1 + 1 + 1 + 5 = 72
    }
    
    #[test]
    fn test_plugin_entry_serialization() {
        let program_id = Pubkey::default();
        let config_account = Pubkey::default();
        let entry = PluginEntry::new(
            program_id,
            config_account,
            PluginType::SolLimit,
            5,
            false,
        );
        
        let bytes = entry.into_bytes().unwrap();
        assert_eq!(bytes.len(), PluginEntry::LEN);
        
        // Deserialize
        let loaded = unsafe { PluginEntry::load_unchecked(bytes).unwrap() };
        assert_eq!(*loaded, entry);
    }
}
