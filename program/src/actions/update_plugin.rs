//! Update Plugin instruction handler - Hybrid Architecture

use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    plugin::PluginEntry, wallet_account::WalletAccount, Discriminator, Transmutable,
};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, ProgramResult};

use crate::error::LazorkitError;
use crate::util::permission::check_role_permission_for_plugin_management;

/// Updates a plugin in the wallet's plugin registry (Hybrid Architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. smart_wallet (signer)
/// 2. acting_authority (for authentication)
/// Format: [acting_authority_id: u32, plugin_index: u16, enabled: u8, priority: u8, padding: [u8; 2]]
pub fn update_plugin(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
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
    // Format: [acting_authority_id: u32 (4 bytes), plugin_index: u16 (2 bytes), enabled: u8 (1 byte), priority: u8 (1 byte), padding: [u8; 2] (2 bytes)]
    // Total: 10 bytes
    if instruction_data.len() < 10 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse acting_authority_id (first 4 bytes)
    let acting_authority_id = u32::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
    ]);

    // Parse args manually to avoid alignment issues
    let plugin_index = u16::from_le_bytes([instruction_data[4], instruction_data[5]]);
    let enabled = instruction_data[6];
    let priority = instruction_data[7];
    // padding at [8..10] - ignore

    // Validate enabled value (must be 0 or 1)
    if enabled > 1 {
        return Err(LazorkitError::InvalidPluginEntry.into());
    }

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

    // Step 4: Execute action (update plugin)
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
    // PluginEntry layout: program_id (32) + config_account (32) + enabled (1) + priority (1) + padding (6)
    // Offsets: program_id (0-31), config_account (32-63), enabled (64), priority (65)
    wallet_account_mut_data[plugin_entry_offset + 64] = enabled; // enabled byte (offset 64)
    wallet_account_mut_data[plugin_entry_offset + 65] = priority; // priority byte (offset 65)

    Ok(())
}
