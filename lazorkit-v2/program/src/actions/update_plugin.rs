//! Update Plugin instruction handler - Pure External Architecture

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    ProgramResult,
};
use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    plugin::PluginEntry,
    Discriminator,
    Transmutable,
};

use crate::error::LazorkitError;

/// Arguments for UpdatePlugin instruction (Pure External)
/// Note: instruction discriminator is already parsed in process_action
#[repr(C, align(8))]
#[derive(Debug)]
pub struct UpdatePluginArgs {
    pub plugin_index: u16,  // Index of plugin to update
    pub enabled: u8,  // New enabled status (0 or 1)
    pub priority: u8,  // New priority
    pub _padding: [u8; 4],
}

impl UpdatePluginArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for UpdatePluginArgs {
    const LEN: usize = Self::LEN;
}

/// Updates a plugin in the wallet's plugin registry (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. payer (writable, signer) - not used, but kept for consistency
/// 2. system_program
pub fn update_plugin(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 3 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }
    
    let wallet_account_info = &accounts[0];
    let _payer = &accounts[1];
    let system_program = &accounts[2];
    
    // Validate system program
    if system_program.key() != &pinocchio_system::ID {
        return Err(LazorkitError::InvalidSystemProgram.into());
    }
    
    // Validate wallet account
    check_self_owned(wallet_account_info, LazorkitError::OwnerMismatchWalletState)?;
    
    let wallet_account_data = unsafe { wallet_account_info.borrow_data_unchecked() };
    if wallet_account_data.is_empty() || wallet_account_data[0] != Discriminator::WalletAccount as u8 {
        return Err(LazorkitError::InvalidWalletStateDiscriminator.into());
    }
    
    let wallet_account = unsafe {
        WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])?
    };
    
    // Parse instruction args
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    if instruction_data.len() < UpdatePluginArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    // Parse args manually to avoid alignment issues
    let plugin_index = u16::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
    ]);
    let enabled = instruction_data[2];
    let priority = instruction_data[3];
    // padding at [4..8] - ignore
    
    // Validate enabled value (must be 0 or 1)
    if enabled > 1 {
        return Err(LazorkitError::InvalidPluginEntry.into());
    }
    
    // Get plugin registry offset
    let registry_offset = wallet_account.plugin_registry_offset(wallet_account_data)?;
    
    // Get current number of plugins
    if registry_offset + 2 > wallet_account_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let num_plugins = u16::from_le_bytes([
        wallet_account_data[registry_offset],
        wallet_account_data[registry_offset + 1],
    ]);
    
    if plugin_index >= num_plugins {
        return Err(LazorkitError::InvalidPluginEntry.into());
    }
    
    // Calculate plugin entry offset
    let plugin_entry_offset = registry_offset + 2 + (plugin_index as usize * PluginEntry::LEN);
    
    if plugin_entry_offset + PluginEntry::LEN > wallet_account_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Get mutable access
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };
    
    // Update plugin entry fields (enabled and priority)
    // PluginEntry layout: program_id (32) + config_account (32) + plugin_type (1) + enabled (1) + priority (1) + padding (5)
    // Offsets: program_id (0-31), config_account (32-63), plugin_type (64), enabled (65), priority (66)
    wallet_account_mut_data[plugin_entry_offset + 65] = enabled;  // enabled byte (offset 65)
    wallet_account_mut_data[plugin_entry_offset + 66] = priority;  // priority byte (offset 66)
    
    Ok(())
}
