//! Plugin reference structure

use crate::{IntoBytes, Transmutable};
use no_padding::NoPadding;
use pinocchio::program_error::ProgramError;

/// Plugin reference - Links authority to a plugin in the registry
///
/// Instead of storing inline permissions, we store references
/// to external plugins.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, NoPadding)]
pub struct PluginRef {
    /// Index trong plugin registry
    pub plugin_index: u16, // 2 bytes
    /// Priority (0 = highest)
    pub priority: u8, // 1 byte
    /// Enabled flag (1 = enabled, 0 = disabled)
    pub enabled: u8, // 1 byte
    /// Padding
    pub _padding: [u8; 4], // 4 bytes
}

impl PluginRef {
    pub const LEN: usize = core::mem::size_of::<Self>();

    /// Create a new PluginRef
    pub fn new(plugin_index: u16, priority: u8, enabled: bool) -> Self {
        Self {
            plugin_index,
            priority,
            enabled: if enabled { 1 } else { 0 },
            _padding: [0; 4],
        }
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled == 1
    }
}

impl Transmutable for PluginRef {
    const LEN: usize = Self::LEN;
}

impl IntoBytes for PluginRef {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        let bytes =
            unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) };
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_ref_creation() {
        let plugin_ref = PluginRef::new(5, 10, true);
        assert_eq!(plugin_ref.plugin_index, 5);
        assert_eq!(plugin_ref.priority, 10);
        assert!(plugin_ref.is_enabled());

        let disabled = PluginRef::new(3, 0, false);
        assert!(!disabled.is_enabled());
    }

    #[test]
    fn test_plugin_ref_size() {
        assert_eq!(PluginRef::LEN, 8);
    }

    #[test]
    fn test_plugin_ref_serialization() {
        let plugin_ref = PluginRef::new(5, 10, true);
        let bytes = plugin_ref.into_bytes().unwrap();
        assert_eq!(bytes.len(), PluginRef::LEN);

        // Deserialize
        let loaded = unsafe { PluginRef::load_unchecked(bytes).unwrap() };
        assert_eq!(*loaded, plugin_ref);
    }
}
