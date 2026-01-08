//! Add Plugin instruction handler - Pure External Architecture

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;
use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    plugin::PluginEntry,
    Discriminator,
    Transmutable,
};

use crate::error::LazorkitError;

/// Arguments for AddPlugin instruction (Pure External)
/// Note: instruction discriminator is already parsed in process_action
#[repr(C, align(8))]
#[derive(Debug)]
pub struct AddPluginArgs {
    pub program_id: Pubkey,        // 32 bytes
    pub config_account: Pubkey,    // 32 bytes
    pub plugin_type: u8,           // 1 byte
    pub enabled: u8,               // 1 byte
    pub priority: u8,              // 1 byte
    pub _padding: [u8; 5],         // 5 bytes (total: 72 bytes = PluginEntry::LEN)
}

impl AddPluginArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for AddPluginArgs {
    const LEN: usize = Self::LEN;
}

/// Adds a plugin to the wallet's plugin registry (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. payer (writable, signer)
/// 2. system_program
pub fn add_plugin(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 3 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }
    
    let wallet_account_info = &accounts[0];
    let payer = &accounts[1];
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
    
    // Parse instruction args manually to avoid alignment issues
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    // AddPluginArgs should be 72 bytes: program_id (32) + config_account (32) + plugin_type (1) + enabled (1) + priority (1) + padding (5)
    const EXPECTED_ARGS_LEN: usize = 72;
    if instruction_data.len() < EXPECTED_ARGS_LEN {
        return Err(LazorkitError::DebugAddPluginDataLength.into());
    }
    
    // Parse PluginEntry fields manually (72 bytes total)
    // program_id: [0..32]
    // config_account: [32..64]
    // plugin_type: [64]
    // enabled: [65]
    // priority: [66]
    // padding: [67..72]
    if instruction_data.len() < 64 {
        return Err(LazorkitError::DebugAddPluginDataLength.into());
    }
    
    // Parse pubkeys using the same method as wallet_account.rs
    let mut program_id_bytes = [0u8; 32];
    program_id_bytes.copy_from_slice(&instruction_data[0..32]);
    let program_id = Pubkey::try_from(program_id_bytes.as_ref())
        .map_err(|_| -> ProgramError { LazorkitError::DebugAddPluginPubkeyParse.into() })?;
    
    let mut config_account_bytes = [0u8; 32];
    config_account_bytes.copy_from_slice(&instruction_data[32..64]);
    let config_account = Pubkey::try_from(config_account_bytes.as_ref())
        .map_err(|_| -> ProgramError { LazorkitError::DebugAddPluginPubkeyParse.into() })?;
    
    if instruction_data.len() < 72 {
        return Err(LazorkitError::DebugAddPluginDataLength.into());
    }
    
    let plugin_type = instruction_data[64];
    let enabled = instruction_data[65];
    let priority = instruction_data[66];
    // padding at [67..72] - ignore
    
    // Get plugin registry offset
    let registry_offset = wallet_account.plugin_registry_offset(wallet_account_data)
        .map_err(|e: ProgramError| -> ProgramError { LazorkitError::DebugAddPluginRegistryOffset.into() })?;
    
    // Get current number of plugins
    if registry_offset + 2 > wallet_account_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let num_plugins = u16::from_le_bytes([
        wallet_account_data[registry_offset],
        wallet_account_data[registry_offset + 1],
    ]);
    
    // Check if plugin already exists (skip if no plugins exist yet)
    if num_plugins > 0 {
        let existing_plugins = wallet_account.get_plugins(wallet_account_data)
            .map_err(|_| -> ProgramError { LazorkitError::DebugAddPluginGetPlugins.into() })?;
    for existing in &existing_plugins {
            if existing.program_id == program_id && existing.config_account == config_account {
            return Err(LazorkitError::DuplicateAuthority.into());
        }
        }
    }
    
    // Calculate new size
    let current_plugins_size = num_plugins as usize * PluginEntry::LEN;
    let new_plugins_size = current_plugins_size + PluginEntry::LEN;
    let new_total_size = registry_offset + 2 + new_plugins_size;
    
    // Calculate aligned size
    let new_total_size_aligned = core::alloc::Layout::from_size_align(
        new_total_size,
        8,
    )
    .map_err(|_| LazorkitError::InvalidAlignment)?
    .pad_to_align()
    .size();
    
    // Resize account if needed
    let current_size = wallet_account_data.len();
    if new_total_size_aligned > current_size {
        wallet_account_info.resize(new_total_size_aligned)?;
        
        // Transfer additional lamports if needed
        let current_lamports = wallet_account_info.lamports();
        let required_lamports = Rent::get()?.minimum_balance(new_total_size_aligned);
        let lamports_needed = required_lamports.saturating_sub(current_lamports);
        
        if lamports_needed > 0 {
            Transfer {
                from: payer,
                to: wallet_account_info,
                lamports: lamports_needed,
            }
            .invoke()?;
        }
    }
    
    // Re-borrow data after potential resize
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };
    
    // Create plugin entry (we'll write it manually, so no need to create struct)
    
    // Write plugin entry manually to avoid alignment issues
    let plugins_data = &mut wallet_account_mut_data[registry_offset + 2..];
    let new_plugin_offset = current_plugins_size;
    
    // Write program_id (32 bytes)
    plugins_data[new_plugin_offset..new_plugin_offset + 32]
        .copy_from_slice(program_id.as_ref());
    
    // Write config_account (32 bytes)
    plugins_data[new_plugin_offset + 32..new_plugin_offset + 64]
        .copy_from_slice(config_account.as_ref());
    
    // Write plugin_type (1 byte)
    plugins_data[new_plugin_offset + 64] = plugin_type;
    
    // Write enabled (1 byte)
    plugins_data[new_plugin_offset + 65] = enabled;
    
    // Write priority (1 byte)
    plugins_data[new_plugin_offset + 66] = priority;
    
    // Write padding (5 bytes) - already zero-initialized
    
    // Update num_plugins count
    let new_num_plugins = num_plugins.wrapping_add(1);
    wallet_account_mut_data[registry_offset..registry_offset + 2]
        .copy_from_slice(&new_num_plugins.to_le_bytes());
    
    Ok(())
}
