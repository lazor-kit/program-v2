//! Add Plugin instruction handler - Hybrid Architecture

use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    plugin::PluginEntry, wallet_account::WalletAccount, Discriminator, IntoBytes, Transmutable,
};

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::error::LazorkitError;
use crate::util::permission::check_role_permission_for_plugin_management;

/// Arguments for AddPlugin instruction (Hybrid Architecture)
/// Note: instruction discriminator is already parsed in process_action
/// Format: [acting_authority_id: u32, program_id: Pubkey, config_account: Pubkey, enabled: u8, priority: u8, padding: [u8; 2]]
pub fn add_plugin(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }

    let wallet_account_info = &accounts[0];
    let payer = &accounts[1];
    let system_program = &accounts[2];
    // accounts[3] is acting_authority (for authentication)

    // Validate system program
    if system_program.key() != &pinocchio_system::ID {
        return Err(LazorkitError::InvalidSystemProgram.into());
    }

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

    // Parse instruction args manually to avoid alignment issues
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    // Format: [acting_authority_id: u32 (4 bytes), program_id: Pubkey (32 bytes), config_account: Pubkey (32 bytes), enabled: u8 (1 byte), priority: u8 (1 byte), padding: [u8; 2] (2 bytes)]
    // Total: 4 + 32 + 32 + 1 + 1 + 2 = 72 bytes
    const MIN_ARGS_LEN: usize = 4 + 32 + 32 + 1 + 1 + 2; // 72 bytes
    if instruction_data.len() < MIN_ARGS_LEN {
        return Err(LazorkitError::DebugAddPluginDataLength.into());
    }

    // Parse acting_authority_id (first 4 bytes)
    let acting_authority_id = u32::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
    ]);

    // Parse PluginEntry fields manually
    // program_id: [4..36]
    // config_account: [36..68]
    // enabled: [68]
    // priority: [69]
    // padding: [70..72]

    // Parse pubkeys using the same method as wallet_account.rs
    let mut program_id_bytes = [0u8; 32];
    program_id_bytes.copy_from_slice(&instruction_data[4..36]);
    let program_id = Pubkey::try_from(program_id_bytes.as_ref())
        .map_err(|_| -> ProgramError { LazorkitError::DebugAddPluginPubkeyParse.into() })?;

    let mut config_account_bytes = [0u8; 32];
    config_account_bytes.copy_from_slice(&instruction_data[36..68]);
    let config_account = Pubkey::try_from(config_account_bytes.as_ref())
        .map_err(|_| -> ProgramError { LazorkitError::DebugAddPluginPubkeyParse.into() })?;

    let enabled = instruction_data[68];
    let priority = instruction_data[69];
    // padding at [70..72] - ignore

    // HYBRID ARCHITECTURE: Authenticate and check inline role permission
    // Step 1: Get acting authority data
    let acting_authority_data = wallet_account
        .get_authority(wallet_account_data, acting_authority_id)?
        .ok_or(LazorkitError::InvalidAuthorityNotFoundByRoleId)?;

    // Step 2: Authenticate acting authority (verify signature)
    let authority_payload = accounts
        .get(3)
        .map(|acc| unsafe { acc.borrow_data_unchecked() });
    crate::util::authenticate::authenticate_authority(
        &acting_authority_data,
        accounts,
        authority_payload,
        Some(instruction_data),
    )?;

    // Step 3: Check inline role permission (All permission required for plugin management)
    check_role_permission_for_plugin_management(&acting_authority_data)?;

    // Get plugin registry offset (current, after any authority additions)
    let registry_offset = wallet_account
        .plugin_registry_offset(wallet_account_data)
        .map_err(|e: ProgramError| -> ProgramError {
            LazorkitError::DebugAddPluginRegistryOffset.into()
        })?;

    // CRITICAL: Get existing plugins using get_plugins
    // This will find plugins even if registry_offset changed after adding authorities
    // However, get_plugins uses the current registry_offset, so if plugins are at an old offset,
    // we need to handle that case separately
    let existing_plugins = wallet_account
        .get_plugins(wallet_account_data)
        .unwrap_or_default();
    let num_existing_plugins = existing_plugins.len() as u16;

    // If get_plugins found plugins, they are valid - use them
    // If get_plugins returned empty but we suspect plugins exist at old offset,
    // we would need to scan, but that's complex and error-prone.
    // Instead, we'll rely on get_plugins and handle the case where plugins need to be moved
    // when registry_offset changes.

    // Check if plugin registry exists at the current offset
    let (actual_registry_offset, num_plugins) = if registry_offset + 2 > wallet_account_data.len() {
        // Current offset is beyond data - plugin registry doesn't exist yet
        // But if we found existing plugins, they must be at an old offset
        (registry_offset, num_existing_plugins)
    } else {
        let raw_num_plugins = u16::from_le_bytes([
            wallet_account_data[registry_offset],
            wallet_account_data[registry_offset + 1],
        ]);

        // Check if num_plugins is valid (not garbage data)
        if raw_num_plugins > 1000 {
            // This might be garbage data - use existing plugins count instead
            (registry_offset, num_existing_plugins)
        } else if raw_num_plugins != num_existing_plugins && num_existing_plugins > 0 {
            // Mismatch: num_plugins at current offset doesn't match existing plugins count
            // This means plugins are at an old offset
            (registry_offset, num_existing_plugins)
        } else {
            // Valid num_plugins, matches existing plugins count
            (registry_offset, raw_num_plugins)
        }
    };

    // Check if plugin already exists (skip if no plugins exist yet)
    if num_plugins > 0 {
        let existing_plugins = wallet_account
            .get_plugins(wallet_account_data)
            .map_err(|_| -> ProgramError { LazorkitError::DebugAddPluginGetPlugins.into() })?;
        for existing in &existing_plugins {
            if existing.program_id == program_id && existing.config_account == config_account {
                return Err(LazorkitError::DuplicateAuthority.into());
            }
        }
    }

    // CRITICAL: Save existing plugins data BEFORE resize (if any)
    // If we found existing plugins via get_plugins, serialize them to preserve them during resize
    let existing_plugins_data = if num_plugins > 0 {
        // Serialize existing plugins to preserve them during resize
        let mut existing_data = Vec::with_capacity(num_plugins as usize * PluginEntry::LEN);
        for (idx, plugin) in existing_plugins.iter().enumerate() {
            let plugin_bytes = plugin.into_bytes()?;
            existing_data.extend_from_slice(plugin_bytes);
        }
        Some(existing_data)
    } else {
        None
    };

    // Calculate new size using actual_registry_offset
    let current_plugins_size = num_plugins as usize * PluginEntry::LEN;
    let new_plugins_size = current_plugins_size + PluginEntry::LEN;
    let new_total_size = actual_registry_offset + 2 + new_plugins_size;

    // Calculate aligned size
    let new_total_size_aligned = core::alloc::Layout::from_size_align(new_total_size, 8)
        .map_err(|_| LazorkitError::InvalidAlignment)?
        .pad_to_align()
        .size();

    // Resize account if needed
    let current_size = wallet_account_data.len();
    let was_resized = new_total_size_aligned > current_size;
    if was_resized {
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

    // Recalculate registry_offset after resize to get the final offset
    // This is important because if authorities were added, the offset might have changed
    let final_registry_offset = wallet_account
        .plugin_registry_offset(wallet_account_mut_data)
        .map_err(|e: ProgramError| -> ProgramError {
            LazorkitError::DebugAddPluginRegistryOffset.into()
        })?;

    // CRITICAL: If account was resized AND registry_offset changed, we need to move existing plugins
    if was_resized && final_registry_offset != actual_registry_offset {
        if let Some(existing_data) = existing_plugins_data {
            if final_registry_offset + 2 + existing_data.len() > wallet_account_mut_data.len() {
                return Err(ProgramError::InvalidAccountData);
            }
            // Copy existing plugins to new offset
            wallet_account_mut_data
                [final_registry_offset + 2..final_registry_offset + 2 + existing_data.len()]
                .copy_from_slice(&existing_data);
            // Restore num_plugins at new offset
            wallet_account_mut_data[final_registry_offset..final_registry_offset + 2]
                .copy_from_slice(&num_plugins.to_le_bytes());
            wallet_account_mut_data[final_registry_offset..final_registry_offset + 2]
                .copy_from_slice(&num_plugins.to_le_bytes());
        } else if final_registry_offset + 2 <= wallet_account_mut_data.len() {
            // No existing plugins, but registry exists - ensure num_plugins is 0
            wallet_account_mut_data[final_registry_offset..final_registry_offset + 2]
                .copy_from_slice(&0u16.to_le_bytes());
        }
    } else if was_resized {
        // Account was resized but offset didn't change - just restore existing plugins
        if let Some(existing_data) = existing_plugins_data {
            if final_registry_offset + 2 + existing_data.len() > wallet_account_mut_data.len() {
                return Err(ProgramError::InvalidAccountData);
            }
            wallet_account_mut_data
                [final_registry_offset + 2..final_registry_offset + 2 + existing_data.len()]
                .copy_from_slice(&existing_data);
            // Restore num_plugins
            wallet_account_mut_data[final_registry_offset..final_registry_offset + 2]
                .copy_from_slice(&num_plugins.to_le_bytes());
            // Restore num_plugins
            wallet_account_mut_data[final_registry_offset..final_registry_offset + 2]
                .copy_from_slice(&num_plugins.to_le_bytes());

            // Verify restored plugins
            let verify_offset = final_registry_offset + 2;
            if verify_offset + 32 <= wallet_account_mut_data.len() {
                let verify_program_id = &wallet_account_mut_data[verify_offset..verify_offset + 8];
            }
        }
    }

    // Use the original num_plugins value (which we preserved/restored)
    let actual_num_plugins = num_plugins;
    let actual_current_plugins_size = actual_num_plugins as usize * PluginEntry::LEN;

    // CRITICAL: If plugin registry doesn't exist yet or was uninitialized (num_plugins was 0),
    // we need to initialize num_plugins = 0 first before writing plugin entry
    // But only if we didn't just restore it above
    if actual_num_plugins == 0
        && !was_resized
        && (registry_offset + 2 > wallet_account_data.len()
            || wallet_account_data[registry_offset..registry_offset + 2] != [0, 0])
    {
        // Plugin registry was beyond old data or uninitialized, now it should be within new data after resize
        if registry_offset + 2 > wallet_account_mut_data.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        wallet_account_mut_data[registry_offset..registry_offset + 2]
            .copy_from_slice(&0u16.to_le_bytes());
        wallet_account_mut_data[registry_offset..registry_offset + 2]
            .copy_from_slice(&0u16.to_le_bytes());
    }

    // Create plugin entry (we'll write it manually, so no need to create struct)

    // Step 4: Execute action (add plugin)
    // Write plugin entry manually to avoid alignment issues
    // Use final_registry_offset (which accounts for any changes after resize)
    let plugins_data = &mut wallet_account_mut_data[final_registry_offset + 2..];
    let new_plugin_offset = actual_current_plugins_size;
    let plugins_data = &mut wallet_account_mut_data[final_registry_offset + 2..];
    let new_plugin_offset = actual_current_plugins_size;

    // Write program_id (32 bytes)
    plugins_data[new_plugin_offset..new_plugin_offset + 32].copy_from_slice(program_id.as_ref());

    // Write config_account (32 bytes)
    // Write config_account (32 bytes)
    plugins_data[new_plugin_offset + 32..new_plugin_offset + 64]
        .copy_from_slice(config_account.as_ref());

    // Write enabled (1 byte)
    plugins_data[new_plugin_offset + 64] = enabled;

    // Write priority (1 byte)
    plugins_data[new_plugin_offset + 65] = priority;

    // Write padding (6 bytes) - already zero-initialized

    // Update num_plugins count at final_registry_offset
    let new_num_plugins = actual_num_plugins.wrapping_add(1);

    // Ensure we have space for num_plugins
    if final_registry_offset + 2 > wallet_account_mut_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }

    wallet_account_mut_data[final_registry_offset..final_registry_offset + 2]
        .copy_from_slice(&new_num_plugins.to_le_bytes());

    Ok(())
}
