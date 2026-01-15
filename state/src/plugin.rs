//! Plugin data format and parsing utilities.
//!
//! This module defines the standard format for plugin data storage in LazorKit state.
//! Each plugin is stored sequentially with a header followed by its state blob.

use no_padding::NoPadding;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

use crate::{IntoBytes, Transmutable};

/// Header stored before each plugin's state blob in the role buffer.
///
/// Format (40 bytes):
/// - program_id: [u8; 32] - The plugin program's public key
/// - data_length: u16 - Size of the state_blob in bytes
/// - _padding: u16 - Explicit padding for alignment  
/// - boundary: u32 - Offset to the next plugin (or end of plugins)
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, NoPadding)]
pub struct PluginHeader {
    /// Plugin program ID that will verify this plugin's state
    pub program_id: [u8; 32],
    /// Length of the state blob following this header
    pub data_length: u16,
    /// Explicit padding for 8-byte alignment
    pub _padding: u16,
    /// Offset to the next plugin (from start of actions_data)
    pub boundary: u32,
}

impl PluginHeader {
    /// Size of the plugin header in bytes (40 bytes with explicit padding)
    pub const LEN: usize = 40; // 32 + 2 + 2 + 4 = 40 bytes

    /// Creates a new plugin header
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

impl Transmutable for PluginHeader {
    const LEN: usize = Self::LEN;
}

impl IntoBytes for PluginHeader {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        Ok(unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) })
    }
}

/// View into a plugin's data within the actions buffer
#[derive(Debug)]
pub struct PluginView<'a> {
    /// The plugin header
    pub header: &'a PluginHeader,
    /// The plugin's state blob
    pub state_blob: &'a [u8],
    /// Index of this plugin in the actions array
    pub index: usize,
    /// Offset in the actions_data buffer where this plugin starts
    pub offset: usize,
}

/// Iterator over plugins in actions_data
pub struct PluginIterator<'a> {
    actions_data: &'a [u8],
    cursor: usize,
    index: usize,
}

impl<'a> PluginIterator<'a> {
    /// Creates a new plugin iterator
    pub fn new(actions_data: &'a [u8]) -> Self {
        Self {
            actions_data,
            cursor: 0,
            index: 0,
        }
    }
}

impl<'a> Iterator for PluginIterator<'a> {
    type Item = Result<PluginView<'a>, ProgramError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.actions_data.len() {
            return None;
        }

        // Check if we have enough data for header
        if self.cursor + PluginHeader::LEN > self.actions_data.len() {
            return Some(Err(ProgramError::InvalidAccountData));
        }

        // Parse header
        let header_bytes = &self.actions_data[self.cursor..self.cursor + PluginHeader::LEN];
        let header = unsafe {
            match PluginHeader::load_unchecked(header_bytes) {
                Ok(h) => h,
                Err(e) => return Some(Err(e)),
            }
        };

        // Calculate state blob range
        let blob_start = self.cursor + PluginHeader::LEN;
        let blob_end = blob_start + header.data_length as usize;

        // Check if we have enough data for blob
        if blob_end > self.actions_data.len() {
            return Some(Err(ProgramError::InvalidAccountData));
        }

        let state_blob = &self.actions_data[blob_start..blob_end];

        let plugin_view = PluginView {
            header,
            state_blob,
            index: self.index,
            offset: self.cursor,
        };

        // Move cursor to next plugin
        self.cursor = header.boundary as usize;
        self.index += 1;

        Some(Ok(plugin_view))
    }
}

/// Parses actions_data into individual plugins
pub fn parse_plugins(actions_data: &[u8]) -> PluginIterator {
    PluginIterator::new(actions_data)
}

/// Counts the number of plugins in actions_data
pub fn count_plugins(actions_data: &[u8]) -> Result<u16, ProgramError> {
    let mut count = 0u16;
    for result in parse_plugins(actions_data) {
        result?; // Validate each plugin
        count = count.saturating_add(1);
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_header_size() {
        assert_eq!(PluginHeader::LEN, 40);
        assert_eq!(core::mem::size_of::<PluginHeader>(), 40);
    }

    #[test]
    fn test_parse_empty() {
        let actions_data = [];
        let plugins: Vec<_> = parse_plugins(&actions_data).collect();
        assert_eq!(plugins.len(), 0);
    }

    #[test]
    fn test_parse_single_plugin() {
        let program_id = Pubkey::from([1u8; 32]);
        let state_data = [42u8; 8];

        let header = PluginHeader::new(program_id, 8, 40 + 8);
        let header_bytes = header.into_bytes().unwrap();

        let mut actions_data = Vec::new();
        actions_data.extend_from_slice(header_bytes);
        actions_data.extend_from_slice(&state_data);

        let plugins: Vec<_> = parse_plugins(&actions_data)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].header.program_id(), program_id);
        assert_eq!(plugins[0].state_blob, &state_data);
        assert_eq!(plugins[0].index, 0);
    }

    #[test]
    fn test_count_plugins() {
        let program_id = Pubkey::from([1u8; 32]);
        let state_data = [42u8; 8];

        let header = PluginHeader::new(program_id, 8, 40 + 8);
        let header_bytes = header.into_bytes().unwrap();

        let mut actions_data = Vec::new();
        actions_data.extend_from_slice(header_bytes);
        actions_data.extend_from_slice(&state_data);

        let count = count_plugins(&actions_data).unwrap();
        assert_eq!(count, 1);
    }
}
