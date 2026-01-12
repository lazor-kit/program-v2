//! Remove Plugin instruction handler - Hybrid Architecture

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
// Note: Using unsafe lamports manipulation instead of Transfer to avoid privilege escalation
use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    plugin::PluginEntry, plugin_ref::PluginRef, position::Position, wallet_account::WalletAccount,
    Discriminator, Transmutable,
};

use crate::error::LazorkitError;
use crate::util::permission::check_role_permission_for_plugin_management;

/// Removes a plugin from the wallet's plugin registry (Hybrid Architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. smart_wallet (signer)
/// 2. acting_authority (for authentication)
/// Format: [acting_authority_id: u32, plugin_index: u16, padding: [u8; 2]]
pub fn remove_plugin(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    if accounts.len() < 3 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }

    let wallet_account_info = &accounts[0];
    let _smart_wallet = &accounts[1];
    // accounts[2] is acting_authority (for authentication)

    // Validate wallet account
    check_self_owned(wallet_account_info, LazorkitError::OwnerMismatchWalletState)?;

    let wallet_account_data = unsafe { wallet_account_info.borrow_data_unchecked() };
    if wallet_account_data.is_empty()
        || wallet_account_data[0] != Discriminator::WalletAccount as u8
    {
        return Err(LazorkitError::InvalidWalletStateDiscriminator.into());
    }

    let wallet_account =
        unsafe { WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])? };

    // Parse instruction args
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    // Format: [acting_authority_id: u32 (4 bytes), plugin_index: u16 (2 bytes), padding: [u8; 2] (2 bytes)]
    // Total: 8 bytes
    if instruction_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse acting_authority_id (first 4 bytes)
    let acting_authority_id = u32::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
    ]);

    // Parse plugin_index (next 2 bytes)
    let plugin_index = u16::from_le_bytes([instruction_data[4], instruction_data[5]]);
    // padding at [6..8] - ignore

    // HYBRID ARCHITECTURE: Authenticate and check inline role permission
    // Step 1: Get acting authority data
    let acting_authority_data = wallet_account
        .get_authority(wallet_account_data, acting_authority_id)?
        .ok_or(LazorkitError::InvalidAuthorityNotFoundByRoleId)?;

    // Step 2: Authenticate acting authority (verify signature)
    let authority_payload = accounts
        .get(2)
        .map(|acc| unsafe { acc.borrow_data_unchecked() });
    crate::util::authenticate::authenticate_authority(
        &acting_authority_data,
        accounts,
        authority_payload,
        Some(instruction_data),
    )?;

    // Step 3: Check inline role permission (All permission required for plugin management)
    check_role_permission_for_plugin_management(&acting_authority_data)?;

    // Step 4: Execute action (remove plugin)

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
    let new_account_size_aligned = core::alloc::Layout::from_size_align(new_account_size, 8)
        .map_err(|_| LazorkitError::InvalidAlignment)?
        .pad_to_align()
        .size();

    wallet_account_info.resize(new_account_size_aligned)?;

    // Note: Excess lamports remain in wallet_account (no payer account in this instruction)
    // This is consistent with the instruction definition which doesn't include a payer account

    Ok(())
}
