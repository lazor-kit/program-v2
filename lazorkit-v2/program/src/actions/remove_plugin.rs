//! Remove Plugin instruction handler - Pure External Architecture

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
// Note: Using unsafe lamports manipulation instead of Transfer to avoid privilege escalation
use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    plugin::PluginEntry,
    position::Position,
    plugin_ref::PluginRef,
    Discriminator,
    Transmutable,
};

use crate::error::LazorkitError;

/// Arguments for RemovePlugin instruction (Pure External)
/// Note: instruction discriminator is already parsed in process_action
#[repr(C, align(8))]
#[derive(Debug)]
pub struct RemovePluginArgs {
    pub plugin_index: u16,  // Index of plugin to remove
    pub _padding: [u8; 2],
}

impl RemovePluginArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for RemovePluginArgs {
    const LEN: usize = Self::LEN;
}

/// Removes a plugin from the wallet's plugin registry (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. payer (writable, signer) - to receive refunded lamports
/// 2. system_program
pub fn remove_plugin(
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
    
    // Parse instruction args
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    if instruction_data.len() < RemovePluginArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    // Parse plugin_index manually to avoid alignment issues
    let plugin_index = u16::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
    ]);
    
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
    
    // Get current account size
    let current_size = wallet_account_data.len();
    
    // Calculate new account size
    let new_account_size = current_size - PluginEntry::LEN;
    
    // Get mutable access
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };
    
    // Compact data: shift all plugins after removed one forward
    let data_after_plugin = plugin_entry_offset + PluginEntry::LEN;
    if data_after_plugin < current_size {
        let data_to_move_len = current_size - data_after_plugin;
        // Shift data forward
        wallet_account_mut_data.copy_within(
            data_after_plugin..data_after_plugin + data_to_move_len,
            plugin_entry_offset,
        );
    }
    
    // Update num_plugins
    let new_num_plugins = num_plugins.saturating_sub(1);
    wallet_account_mut_data[registry_offset..registry_offset + 2]
        .copy_from_slice(&new_num_plugins.to_le_bytes());
    
    // CRITICAL: Update plugin_index in all PluginRefs of all authorities
    // When a plugin is removed, all plugin_index > removed_index need to be decremented by 1
    let authorities_offset = wallet_account.authorities_offset();
    let num_authorities = wallet_account.num_authorities(wallet_account_mut_data)?;
    let mut authority_offset = authorities_offset;
    
    for _ in 0..num_authorities {
        if authority_offset + Position::LEN > new_account_size {
            break;
        }
        
        // Parse Position manually
        let position_num_plugin_refs = u16::from_le_bytes([
            wallet_account_mut_data[authority_offset + 4],
            wallet_account_mut_data[authority_offset + 5],
        ]);
        let position_boundary = u32::from_le_bytes([
            wallet_account_mut_data[authority_offset + 12],
            wallet_account_mut_data[authority_offset + 13],
            wallet_account_mut_data[authority_offset + 14],
            wallet_account_mut_data[authority_offset + 15],
        ]);
    
        // Get authority data and plugin refs
        let position_authority_length = u16::from_le_bytes([
            wallet_account_mut_data[authority_offset + 2],
            wallet_account_mut_data[authority_offset + 3],
        ]);
        
        let auth_data_start = authority_offset + Position::LEN;
        let auth_data_end = auth_data_start + position_authority_length as usize;
        let plugin_refs_start = auth_data_end;
        let plugin_refs_end = position_boundary as usize;
        
        // Update plugin_refs
        let mut ref_cursor = plugin_refs_start;
        for _ in 0..position_num_plugin_refs {
            if ref_cursor + PluginRef::LEN > plugin_refs_end {
                break;
            }
            
            // Read current plugin_index
            let current_plugin_index = u16::from_le_bytes([
                wallet_account_mut_data[ref_cursor],
                wallet_account_mut_data[ref_cursor + 1],
            ]);
            
            // Update plugin_index if needed
            if current_plugin_index > plugin_index {
                // Decrement plugin_index
                let new_plugin_index = current_plugin_index.saturating_sub(1);
                wallet_account_mut_data[ref_cursor..ref_cursor + 2]
                    .copy_from_slice(&new_plugin_index.to_le_bytes());
            } else if current_plugin_index == plugin_index {
                // Plugin being removed - disable the ref
                wallet_account_mut_data[ref_cursor + 3] = 0; // Set enabled = 0
            }
            // If current_plugin_index < plugin_index, no change needed
            
            ref_cursor += PluginRef::LEN;
        }
        
        authority_offset = position_boundary as usize;
    }
    
    // Resize account to new size
    let new_account_size_aligned = core::alloc::Layout::from_size_align(
        new_account_size,
        8,
    )
    .map_err(|_| LazorkitError::InvalidAlignment)?
    .pad_to_align()
    .size();
    
    wallet_account_info.resize(new_account_size_aligned)?;
    
    // Refund excess lamports to payer (using unsafe like Swig)
    let current_lamports = unsafe { *wallet_account_info.borrow_lamports_unchecked() };
    let required_lamports = Rent::get()?.minimum_balance(new_account_size_aligned);
    let excess_lamports = current_lamports.saturating_sub(required_lamports);
    
    if excess_lamports > 0 {
        unsafe {
            *wallet_account_info.borrow_mut_lamports_unchecked() = current_lamports - excess_lamports;
            *payer.borrow_mut_lamports_unchecked() = payer.lamports() + excess_lamports;
        }
    }
    
    Ok(())
}
