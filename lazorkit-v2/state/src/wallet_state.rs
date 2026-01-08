//! Wallet State account structure.

use crate::{Discriminator, Transmutable, TransmutableMut, IntoBytes};
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};
use crate::plugin::PluginEntry;

/// Wallet State account structure.
///
/// This account stores the configuration and execution state of a Lazorkit smart wallet.
#[repr(C, align(8))]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct WalletState {
    pub discriminator: u8,
    pub bump: u8,
    pub last_nonce: u64,
    pub base_seed: [u8; 32],
    pub salt: u64,
    
    // Plugin registry header
    pub num_plugins: u16,
    pub _padding: [u8; 2],  // Padding to align to 8 bytes
    
    // Dynamic: Plugin entries follow after this struct in account data
    // plugins: Vec<PluginEntry>
}

impl WalletState {
    /// Size of the fixed header (without dynamic plugins)
    pub const LEN: usize = core::mem::size_of::<Self>();
    
    /// PDA seed prefix for WalletState
    pub const PREFIX_SEED: &'static [u8] = b"wallet_state";
    
    /// Smart wallet seed prefix
    pub const SMART_WALLET_SEED: &'static [u8] = b"smart_wallet";
    
    /// Create a new WalletState
    pub fn new(
        base_seed: [u8; 32],
        salt: u64,
        bump: u8,
    ) -> Self {
        Self {
            discriminator: Discriminator::WalletState as u8,
            bump,
            last_nonce: 0,
            base_seed,
            salt,
            num_plugins: 0,
            _padding: [0; 2],
        }
    }
    
    /// Get plugin entries from account data
    pub fn get_plugins(&self, account_data: &[u8]) -> Result<Vec<PluginEntry>, ProgramError> {
        if account_data.len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        
        let plugins_data = &account_data[Self::LEN..];
        let mut plugins = Vec::new();
        let mut cursor = 0;
        let entry_size = PluginEntry::LEN;
        
        for _ in 0..self.num_plugins {
            if cursor + entry_size > plugins_data.len() {
                return Err(ProgramError::InvalidAccountData);
            }
            
            let entry = unsafe {
                PluginEntry::load_unchecked(&plugins_data[cursor..cursor + entry_size])?
            };
            plugins.push(*entry);
            cursor += entry_size;
        }
        
        Ok(plugins)
    }
    
    /// Get enabled plugins sorted by priority
    pub fn get_enabled_plugins(&self, account_data: &[u8]) -> Result<Vec<PluginEntry>, ProgramError> {
        let mut plugins = self.get_plugins(account_data)?;
        plugins.retain(|p| p.enabled == 1);
        plugins.sort_by_key(|p| p.priority);
        Ok(plugins)
    }
    
    /// Add a plugin to the registry
    pub fn add_plugin(
        &mut self,
        account_data: &mut [u8],
        plugin: PluginEntry,
    ) -> Result<(), ProgramError> {
        // Check if plugin already exists
        let existing_plugins = self.get_plugins(account_data)?;
        for existing in &existing_plugins {
            if existing.program_id == plugin.program_id && existing.config_account == plugin.config_account {
                return Err(ProgramError::InvalidAccountData); // Duplicate plugin
            }
        }
        
        // Calculate new size
        let current_plugins_size = self.num_plugins as usize * PluginEntry::LEN;
        let new_plugins_size = current_plugins_size + PluginEntry::LEN;
        let new_total_size = Self::LEN + new_plugins_size;
        
        // Ensure account data is large enough
        if account_data.len() < new_total_size {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Append plugin entry
        let plugins_data = &mut account_data[Self::LEN..];
        let plugin_bytes = plugin.into_bytes()?;
        plugins_data[current_plugins_size..current_plugins_size + PluginEntry::LEN]
            .copy_from_slice(plugin_bytes);
        
        // Update count
        self.num_plugins += 1;
        
        Ok(())
    }
    
    /// Remove a plugin from the registry by index
    pub fn remove_plugin_by_index(
        &mut self,
        account_data: &mut [u8],
        index: usize,
    ) -> Result<(), ProgramError> {
        if index >= self.num_plugins as usize {
            return Err(ProgramError::InvalidAccountData);
        }
        
        let plugins_data = &mut account_data[Self::LEN..];
        let entry_size = PluginEntry::LEN;
        let current_plugins_size = self.num_plugins as usize * entry_size;
        
        // Calculate removal position
        let remove_offset = index * entry_size;
        let remaining_size = current_plugins_size - remove_offset - entry_size;
        
        // Shift remaining plugins left
        if remaining_size > 0 {
            let source_start = remove_offset + entry_size;
            let source_end = source_start + remaining_size;
            let dest_start = remove_offset;
            let _dest_end = dest_start + remaining_size;
            
            // Use copy_within to avoid borrow conflicts
            plugins_data.copy_within(source_start..source_end, dest_start);
        }
        
        // Zero out the last entry
        if current_plugins_size >= entry_size {
            plugins_data[current_plugins_size - entry_size..current_plugins_size].fill(0);
        }
        
        // Update count
        self.num_plugins -= 1;
        
        Ok(())
    }
    
    /// Update a plugin in the registry by index
    pub fn update_plugin_by_index(
        &mut self,
        account_data: &mut [u8],
        index: usize,
        plugin: PluginEntry,
    ) -> Result<(), ProgramError> {
        if index >= self.num_plugins as usize {
            return Err(ProgramError::InvalidAccountData);
        }
        
        let plugins_data = &mut account_data[Self::LEN..];
        let entry_size = PluginEntry::LEN;
        let update_offset = index * entry_size;
        
        // Update plugin entry
        let plugin_bytes = plugin.into_bytes()?;
        plugins_data[update_offset..update_offset + entry_size]
            .copy_from_slice(plugin_bytes);
        
        Ok(())
    }
    
    /// Find plugin index by program_id and config_account
    pub fn find_plugin_index(
        &self,
        account_data: &[u8],
        program_id: &Pubkey,
        config_account: &Pubkey,
    ) -> Result<Option<usize>, ProgramError> {
        let plugins = self.get_plugins(account_data)?;
        for (index, plugin) in plugins.iter().enumerate() {
            if plugin.program_id == *program_id && plugin.config_account == *config_account {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
}

impl Transmutable for WalletState {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for WalletState {}

impl IntoBytes for WalletState {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        let bytes = unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN)
        };
        Ok(bytes)
    }
}

/// Helper functions for PDA derivation
pub fn wallet_state_seeds(smart_wallet: &Pubkey) -> [&[u8]; 2] {
    [WalletState::PREFIX_SEED, smart_wallet.as_ref()]
}

pub fn wallet_state_seeds_with_bump<'a>(smart_wallet: &'a Pubkey, bump: &'a [u8]) -> [&'a [u8]; 3] {
    [WalletState::PREFIX_SEED, smart_wallet.as_ref(), bump]
}

/// Creates a signer seeds array for a WalletState account.
pub fn wallet_state_signer<'a>(
    smart_wallet: &'a Pubkey,
    bump: &'a [u8; 1],
) -> [pinocchio::instruction::Seed<'a>; 3] {
    [
        WalletState::PREFIX_SEED.into(),
        smart_wallet.as_ref().into(),
        bump.as_ref().into(),
    ]
}

pub fn smart_wallet_seeds<'a>(base_seed: &'a [u8], salt_bytes: &'a [u8; 8]) -> [&'a [u8]; 3] {
    [
        WalletState::SMART_WALLET_SEED,
        base_seed,
        salt_bytes,
    ]
}

pub fn smart_wallet_seeds_with_bump<'a>(
    base_seed: &'a [u8],
    salt_bytes: &'a [u8; 8],
    bump: &'a [u8],
) -> [&'a [u8]; 4] {
    [
        WalletState::SMART_WALLET_SEED,
        base_seed,
        salt_bytes,
        bump,
    ]
}

/// Creates a signer seeds array for a Smart Wallet account.
/// Note: salt_bytes must be provided by caller to avoid lifetime issues
pub fn smart_wallet_signer<'a>(
    base_seed: &'a [u8],
    salt_bytes: &'a [u8; 8],
    bump: &'a [u8; 1],
) -> [pinocchio::instruction::Seed<'a>; 4] {
    [
        WalletState::SMART_WALLET_SEED.into(),
        base_seed.into(),
        salt_bytes.as_ref().into(),
        bump.as_ref().into(),
    ]
}
