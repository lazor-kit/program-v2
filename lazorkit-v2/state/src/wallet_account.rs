//! Wallet Account structure - Swig-like design với external plugins

use crate::{Discriminator, Transmutable, TransmutableMut, IntoBytes};
use no_padding::NoPadding;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};
use crate::position::Position;
use crate::plugin::PluginEntry;
use crate::plugin_ref::PluginRef;

/// Wallet Account - Main account structure (Swig-like)
///
/// Stores all authorities and plugins in a single account for cost efficiency.
/// Layout: 1 (discriminator) + 1 (bump) + 32 (id) + 1 (wallet_bump) + 1 (version) + 4 (padding) = 40 bytes
#[repr(C, align(8))]
#[derive(Debug, PartialEq, Copy, Clone, NoPadding)]
pub struct WalletAccount {
    /// Account type discriminator
    pub discriminator: u8,      // 1 byte
    /// PDA bump seed
    pub bump: u8,               // 1 byte
    /// Unique wallet identifier
    pub id: [u8; 32],           // 32 bytes
    /// Wallet vault PDA bump seed
    pub wallet_bump: u8,        // 1 byte
    /// Account version
    pub version: u8,            // 1 byte
    /// Reserved for future use (padding to align to 8 bytes)
    pub _reserved: [u8; 4],     // 4 bytes (total: 40 bytes, aligned to 8)
}

impl WalletAccount {
    /// Size of the fixed header (without dynamic data)
    pub const LEN: usize = core::mem::size_of::<Self>();
    
    /// PDA seed prefix for WalletAccount
    pub const PREFIX_SEED: &'static [u8] = b"wallet_account";
    
    /// Wallet vault seed prefix
    pub const WALLET_VAULT_SEED: &'static [u8] = b"wallet_vault";
    
    /// Create a new WalletAccount
    pub fn new(id: [u8; 32], bump: u8, wallet_bump: u8) -> Self {
        Self {
            discriminator: Discriminator::WalletAccount as u8,
            bump,
            id,
            wallet_bump,
            version: 1,
            _reserved: [0; 4],
        }
    }
    
    /// Get number of authorities
    pub fn num_authorities(&self, account_data: &[u8]) -> Result<u16, ProgramError> {
        if account_data.len() < Self::LEN + 2 {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(u16::from_le_bytes([
            account_data[Self::LEN],
            account_data[Self::LEN + 1],
        ]))
    }
    
    /// Set number of authorities
    pub fn set_num_authorities(&self, account_data: &mut [u8], num: u16) -> Result<(), ProgramError> {
        if account_data.len() < Self::LEN + 2 {
            return Err(ProgramError::InvalidAccountData);
        }
        account_data[Self::LEN..Self::LEN + 2].copy_from_slice(&num.to_le_bytes());
        Ok(())
    }
    
    /// Get authorities section offset
    pub fn authorities_offset(&self) -> usize {
        Self::LEN + 2  // After num_authorities (2 bytes)
    }
    
    /// Get plugin registry offset
    pub fn plugin_registry_offset(&self, account_data: &[u8]) -> Result<usize, ProgramError> {
        let mut offset = self.authorities_offset();
        
        // Skip authorities
        let num_auths = self.num_authorities(account_data)?;
        for _ in 0..num_auths {
            if offset + Position::LEN > account_data.len() {
                return Err(ProgramError::InvalidAccountData);
            }
            // Parse Position boundary manually to avoid alignment issues
            let position_boundary = u32::from_le_bytes([
                account_data[offset + 12],
                account_data[offset + 13],
                account_data[offset + 14],
                account_data[offset + 15],
            ]);
            offset = position_boundary as usize;
        }
        
        Ok(offset)
    }
    
    /// Get plugin entries from registry
    pub fn get_plugins(&self, account_data: &[u8]) -> Result<Vec<PluginEntry>, ProgramError> {
        let offset = self.plugin_registry_offset(account_data)?;
        if offset + 2 > account_data.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        
        let num_plugins = u16::from_le_bytes([
            account_data[offset],
            account_data[offset + 1],
        ]);
        
        let mut plugins = Vec::new();
        let mut cursor = offset + 2;
        
        for _ in 0..num_plugins {
            if cursor + PluginEntry::LEN > account_data.len() {
                return Err(ProgramError::InvalidAccountData);
            }
            
            // Parse PluginEntry manually to avoid alignment issues
            // PluginEntry layout: program_id (32) + config_account (32) + plugin_type (1) + enabled (1) + priority (1) + padding (5) = 72 bytes
            let mut program_id_bytes = [0u8; 32];
            program_id_bytes.copy_from_slice(&account_data[cursor..cursor + 32]);
            let program_id = Pubkey::try_from(program_id_bytes.as_ref())
                .map_err(|_| ProgramError::InvalidAccountData)?;
            
            let mut config_account_bytes = [0u8; 32];
            config_account_bytes.copy_from_slice(&account_data[cursor + 32..cursor + 64]);
            let config_account = Pubkey::try_from(config_account_bytes.as_ref())
                .map_err(|_| ProgramError::InvalidAccountData)?;
            
            let plugin_type = account_data[cursor + 64];
            let enabled = account_data[cursor + 65];
            let priority = account_data[cursor + 66];
            // padding at cursor + 67..72 - ignore
            
            plugins.push(PluginEntry {
                program_id,
                config_account,
                plugin_type,
                enabled,
                priority,
                _padding: [0; 5],
            });
            cursor += PluginEntry::LEN;
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
    
    /// Get authority by ID
    pub fn get_authority(
        &self,
        account_data: &[u8],
        authority_id: u32,
    ) -> Result<Option<AuthorityData>, ProgramError> {
        let mut offset = self.authorities_offset();
        let num_auths = self.num_authorities(account_data)?;
        
        for _ in 0..num_auths {
            if offset + Position::LEN > account_data.len() {
                break;
            }
            
            // Parse Position manually to avoid alignment issues
            // Position layout: authority_type (2) + authority_length (2) + num_plugin_refs (2) + padding (2) + id (4) + boundary (4)
            if offset + Position::LEN > account_data.len() {
                break;
            }
            
            let position_authority_type = u16::from_le_bytes([
                account_data[offset],
                account_data[offset + 1],
            ]);
            let position_authority_length = u16::from_le_bytes([
                account_data[offset + 2],
                account_data[offset + 3],
            ]);
            let position_num_plugin_refs = u16::from_le_bytes([
                account_data[offset + 4],
                account_data[offset + 5],
            ]);
            let position_id = u32::from_le_bytes([
                account_data[offset + 8],
                account_data[offset + 9],
                account_data[offset + 10],
                account_data[offset + 11],
            ]);
            let position_boundary = u32::from_le_bytes([
                account_data[offset + 12],
                account_data[offset + 13],
                account_data[offset + 14],
                account_data[offset + 15],
            ]);
            
            if position_id == authority_id {
                // Found authority
                let auth_data_start = offset + Position::LEN;
                let auth_data_end = auth_data_start + position_authority_length as usize;
                let plugin_refs_start = auth_data_end;
                let plugin_refs_end = position_boundary as usize;
                
                if plugin_refs_end > account_data.len() {
                    return Err(ProgramError::InvalidAccountData);
                }
                
                let authority_data = account_data[auth_data_start..auth_data_end].to_vec();
                let plugin_refs_data = &account_data[plugin_refs_start..plugin_refs_end];
                
                // Parse plugin refs manually to avoid alignment issues
                let mut plugin_refs = Vec::new();
                let mut ref_cursor = 0;
                for _ in 0..position_num_plugin_refs {
                    if ref_cursor + PluginRef::LEN > plugin_refs_data.len() {
                        break;
                    }
                    // PluginRef layout: plugin_index (2) + priority (1) + enabled (1) + padding (4) = 8 bytes
                    let plugin_index = u16::from_le_bytes([
                        plugin_refs_data[ref_cursor],
                        plugin_refs_data[ref_cursor + 1],
                    ]);
                    let priority = plugin_refs_data[ref_cursor + 2];
                    let enabled = plugin_refs_data[ref_cursor + 3];
                    // padding at 4..8 - ignore
                    
                    plugin_refs.push(PluginRef {
                        plugin_index,
                        priority,
                        enabled,
                        _padding: [0; 4],
                    });
                    ref_cursor += PluginRef::LEN;
                }
                
                // Create Position struct for return
                let position = Position::new(
                    position_authority_type,
                    position_authority_length,
                    position_num_plugin_refs,
                    position_id,
                    position_boundary,
                );
                
                return Ok(Some(AuthorityData {
                    position,
                    authority_data,
                    plugin_refs,
                }));
            }
            
            offset = position_boundary as usize;
        }
        
        Ok(None)
    }
    
    /// Get metadata offset (after plugin registry)
    pub fn metadata_offset(&self, account_data: &[u8]) -> Result<usize, ProgramError> {
        let registry_offset = self.plugin_registry_offset(account_data)?;
        if registry_offset + 2 > account_data.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        
        let num_plugins = u16::from_le_bytes([
            account_data[registry_offset],
            account_data[registry_offset + 1],
        ]);
        
        Ok(registry_offset + 2 + (num_plugins as usize * PluginEntry::LEN))
    }
    
    /// Get last nonce
    pub fn get_last_nonce(&self, account_data: &[u8]) -> Result<u64, ProgramError> {
        let offset = self.metadata_offset(account_data)?;
        if offset + 8 > account_data.len() {
            return Ok(0); // Default to 0 if not set
        }
        
        Ok(u64::from_le_bytes(
            account_data[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?,
        ))
    }
    
    /// Set last nonce
    pub fn set_last_nonce(&self, account_data: &mut [u8], nonce: u64) -> Result<(), ProgramError> {
        let offset = self.metadata_offset(account_data)?;
        if offset + 8 > account_data.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        
        account_data[offset..offset + 8].copy_from_slice(&nonce.to_le_bytes());
        Ok(())
    }
}

impl Transmutable for WalletAccount {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for WalletAccount {}

impl IntoBytes for WalletAccount {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        let bytes = unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN)
        };
        Ok(bytes)
    }
}

/// Authority data structure
pub struct AuthorityData {
    pub position: Position,
    pub authority_data: Vec<u8>,
    pub plugin_refs: Vec<PluginRef>,
}

/// Helper functions for PDA derivation
pub fn wallet_account_seeds(id: &[u8]) -> [&[u8]; 2] {
    [WalletAccount::PREFIX_SEED, id]
}

pub fn wallet_account_seeds_with_bump<'a>(id: &'a [u8], bump: &'a [u8]) -> [&'a [u8]; 3] {
    [WalletAccount::PREFIX_SEED, id, bump]
}

/// Creates a signer seeds array for a WalletAccount PDA (similar to Swig's swig_account_signer)
pub fn wallet_account_signer<'a>(id: &'a [u8], bump: &'a [u8; 1]) -> [pinocchio::instruction::Seed<'a>; 3] {
    [
        WalletAccount::PREFIX_SEED.into(),
        id.as_ref().into(),
        bump.as_ref().into(),
    ]
}

pub fn wallet_vault_seeds(wallet_account_key: &[u8]) -> [&[u8]; 2] {
    [WalletAccount::WALLET_VAULT_SEED, wallet_account_key]
}

pub fn wallet_vault_seeds_with_bump<'a>(
    wallet_account_key: &'a [u8],
    bump: &'a [u8],
) -> [&'a [u8]; 3] {
    [WalletAccount::WALLET_VAULT_SEED, wallet_account_key, bump]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wallet_account_creation() {
        let id = [1u8; 32];
        let bump = 255;
        let wallet_bump = 254;
        
        let wallet = WalletAccount::new(id, bump, wallet_bump);
        
        assert_eq!(wallet.discriminator, Discriminator::WalletAccount as u8);
        assert_eq!(wallet.bump, bump);
        assert_eq!(wallet.id, id);
        assert_eq!(wallet.wallet_bump, wallet_bump);
        assert_eq!(wallet.version, 1);
    }
    
    #[test]
    fn test_wallet_account_size() {
        assert_eq!(WalletAccount::LEN, 40);
    }
    
    #[test]
    fn test_num_authorities_empty() {
        let wallet = WalletAccount::new([0; 32], 0, 0);
        let mut account_data = vec![0u8; WalletAccount::LEN + 2];
        
        // Write wallet account
        let wallet_bytes = wallet.into_bytes().unwrap();
        account_data[..WalletAccount::LEN].copy_from_slice(wallet_bytes);
        
        // Write num_authorities = 0
        account_data[WalletAccount::LEN..WalletAccount::LEN + 2]
            .copy_from_slice(&0u16.to_le_bytes());
        
        let num = wallet.num_authorities(&account_data).unwrap();
        assert_eq!(num, 0);
    }
    
    #[test]
    fn test_get_last_nonce_default() {
        let wallet = WalletAccount::new([0; 32], 0, 0);
        let account_data = vec![0u8; WalletAccount::LEN + 2 + 2 + 8]; // header + num_auths + num_plugins + nonce
        
        // Write wallet account
        let wallet_bytes = wallet.into_bytes().unwrap();
        let mut data = account_data.clone();
        data[..WalletAccount::LEN].copy_from_slice(wallet_bytes);
        data[WalletAccount::LEN..WalletAccount::LEN + 2].copy_from_slice(&0u16.to_le_bytes()); // num_authorities
        data[WalletAccount::LEN + 2..WalletAccount::LEN + 4].copy_from_slice(&0u16.to_le_bytes()); // num_plugins
        
        let nonce = wallet.get_last_nonce(&data).unwrap();
        assert_eq!(nonce, 0);
    }
}
