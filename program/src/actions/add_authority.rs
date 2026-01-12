//! Add Authority instruction handler - Pure External Architecture

use lazorkit_v2_assertions::{check_self_owned, check_system_owner};
use lazorkit_v2_state::{
    authority::AuthorityType, plugin::PluginEntry, plugin_ref::PluginRef, position::Position,
    wallet_account::WalletAccount, Discriminator, IntoBytes, Transmutable, TransmutableMut,
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::error::LazorkitError;
use crate::util::invoke::find_account_info;
use crate::util::permission::check_role_permission_for_authority_management;
use lazorkit_v2_state::role_permission::RolePermission;

/// Arguments for AddAuthority instruction (Hybrid Architecture)
/// Note: instruction discriminator is already parsed in process_action
#[repr(C, align(8))]
#[derive(Debug)]
pub struct AddAuthorityArgs {
    pub acting_authority_id: u32, // Authority ID performing this action (for authentication & permission check)
    pub new_authority_type: u16,
    pub new_authority_data_len: u16,
    pub num_plugin_refs: u16, // Number of plugin refs (usually 0 initially)
    pub role_permission: u8,  // RolePermission enum for new authority (Hybrid: inline permission)
    pub _padding: [u8; 3],    // Padding to align to 8 bytes
}

impl AddAuthorityArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for AddAuthorityArgs {
    const LEN: usize = Self::LEN;
}

/// Adds a new authority to the wallet (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. payer (writable, signer)
/// 2. system_program
pub fn add_authority(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
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
    if wallet_account_data.is_empty()
        || wallet_account_data[0] != Discriminator::WalletAccount as u8
    {
        return Err(LazorkitError::InvalidWalletStateDiscriminator.into());
    }

    let wallet_account =
        unsafe { WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])? };

    // Parse instruction args
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    // So instruction_data here starts after the discriminator
    if instruction_data.len() < AddAuthorityArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse fields manually to avoid alignment issues
    // AddAuthorityArgs: acting_authority_id (4) + new_authority_type (2) + new_authority_data_len (2) + num_plugin_refs (2) + role_permission (1) + padding (3) = 14 bytes (aligned to 8)
    let acting_authority_id = u32::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
    ]);
    let new_authority_type = u16::from_le_bytes([instruction_data[4], instruction_data[5]]);
    let new_authority_data_len = u16::from_le_bytes([instruction_data[6], instruction_data[7]]);
    let num_plugin_refs = u16::from_le_bytes([instruction_data[8], instruction_data[9]]);
    let role_permission_byte = if instruction_data.len() > 10 {
        instruction_data[10]
    } else {
        RolePermission::default() as u8
    };
    let role_permission = RolePermission::try_from(role_permission_byte)
        .map_err(|_| LazorkitError::InvalidRolePermission)?;

    // Parse authority data
    let authority_data_start = AddAuthorityArgs::LEN;
    let authority_data_end = authority_data_start + new_authority_data_len as usize;

    if instruction_data.len() < authority_data_end {
        return Err(ProgramError::InvalidInstructionData);
    }

    let authority_data = &instruction_data[authority_data_start..authority_data_end];

    // Parse plugin refs (if any)
    let plugin_refs_start = authority_data_end;
    let plugin_refs_end = plugin_refs_start + (num_plugin_refs as usize * PluginRef::LEN);
    if instruction_data.len() < plugin_refs_end {
        return Err(ProgramError::InvalidInstructionData);
    }
    let plugin_refs_data = &instruction_data[plugin_refs_start..plugin_refs_end];

    // Validate authority type
    let authority_type = AuthorityType::try_from(new_authority_type)
        .map_err(|_| LazorkitError::InvalidAuthorityType)?;

    // Get acting authority data (authority performing this action)
    // Note: Wallet should always have at least 1 authority (created in create_smart_wallet)
    let acting_authority_data = wallet_account
        .get_authority(wallet_account_data, acting_authority_id)?
        .ok_or(LazorkitError::InvalidAuthorityNotFoundByRoleId)?;

    // Pattern: Authenticate → CPI plugin check permission → Execute
    // Step 1: Authenticate acting authority (verify signature)
    let authority_payload = accounts
        .get(3)
        .map(|acc| unsafe { acc.borrow_data_unchecked() });
    crate::util::authenticate::authenticate_authority(
        &acting_authority_data,
        accounts,
        authority_payload,
        Some(instruction_data),
    )?;

    // HYBRID ARCHITECTURE: Step 2 - Check inline role permission only
    // Only check role permission (4 types: All, ManageAuthority, AllButManageAuthority, ExecuteOnly)
    // No CPI plugin check needed for authority management - inline permission is sufficient
    check_role_permission_for_authority_management(&acting_authority_data)?;

    // Step 3: Check for duplicate authority (same authority_data)
    // Get current account size and calculate new size
    let current_size = wallet_account_data.len();
    let num_authorities = wallet_account.num_authorities(wallet_account_data)?;

    // Check if authority with same data already exists
    let authorities_offset = wallet_account.authorities_offset();
    let mut offset = authorities_offset;
    for _ in 0..num_authorities {
        if offset + Position::LEN > current_size {
            break;
        }

        // Parse Position to get authority_length
        let position_authority_length = u16::from_le_bytes([
            wallet_account_data[offset + 2],
            wallet_account_data[offset + 3],
        ]);
        let position_boundary = u32::from_le_bytes([
            wallet_account_data[offset + 12],
            wallet_account_data[offset + 13],
            wallet_account_data[offset + 14],
            wallet_account_data[offset + 15],
        ]);

        // Get authority data
        let auth_data_start = offset + Position::LEN;
        let auth_data_end = auth_data_start + position_authority_length as usize;

        if auth_data_end <= current_size && position_authority_length == new_authority_data_len {
            let existing_authority_data = &wallet_account_data[auth_data_start..auth_data_end];
            if existing_authority_data == authority_data {
                return Err(LazorkitError::DuplicateAuthority.into());
            }
        }

        offset = position_boundary as usize;
    }

    // Step 4: Execute action (add authority)
    // No CPI plugin check needed - inline permission is sufficient

    // Calculate new authority size
    // Position (16 bytes) + authority_data + plugin_refs
    let plugin_refs_size = num_plugin_refs as usize * PluginRef::LEN;
    let new_authority_size = Position::LEN + new_authority_data_len as usize + plugin_refs_size;

    // Calculate new account size
    let authorities_offset = wallet_account.authorities_offset();
    let new_account_size = current_size + new_authority_size;

    // Reallocate account
    let new_account_size_aligned = core::alloc::Layout::from_size_align(new_account_size, 8)
        .map_err(|_| LazorkitError::InvalidAlignment)?
        .pad_to_align()
        .size();

    // Resize account (Pinocchio uses resize instead of realloc)
    wallet_account_info.resize(new_account_size_aligned)?;

    // Get mutable access after realloc
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };

    // Calculate new authority ID (increment from last authority or start at 0)
    let new_authority_id = if num_authorities == 0 {
        0
    } else {
        // Find last authority to get its ID
        let mut offset = authorities_offset;
        let mut last_id = 0u32;
        for _ in 0..num_authorities {
            if offset + Position::LEN > current_size {
                break;
            }
            // Parse Position manually to avoid alignment issues
            let position_id = u32::from_le_bytes([
                wallet_account_data[offset + 8],
                wallet_account_data[offset + 9],
                wallet_account_data[offset + 10],
                wallet_account_data[offset + 11],
            ]);
            let position_boundary = u32::from_le_bytes([
                wallet_account_data[offset + 12],
                wallet_account_data[offset + 13],
                wallet_account_data[offset + 14],
                wallet_account_data[offset + 15],
            ]);
            last_id = position_id;
            offset = position_boundary as usize;
        }
        last_id.wrapping_add(1)
    };

    // CRITICAL: Get plugin registry from old data BEFORE writing new authority
    // This ensures we preserve it even if it gets overwritten
    let old_registry_offset = wallet_account
        .plugin_registry_offset(wallet_account_data)
        .unwrap_or(current_size);
    let old_plugin_registry_data = if old_registry_offset + 2 <= current_size {
        let old_num_plugins = u16::from_le_bytes([
            wallet_account_data[old_registry_offset],
            wallet_account_data[old_registry_offset + 1],
        ]);
        if old_num_plugins > 0 && old_num_plugins <= 100 {
            let old_plugins_size = old_num_plugins as usize * PluginEntry::LEN;
            let old_registry_size = 2 + old_plugins_size;
            if old_registry_offset + old_registry_size <= current_size {
                Some((
                    old_registry_offset,
                    old_num_plugins,
                    wallet_account_data
                        [old_registry_offset..old_registry_offset + old_registry_size]
                        .to_vec(),
                ))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    if let Some((_, num_plugins, _)) = &old_plugin_registry_data {}

    // Calculate boundary (end of this authority)
    let new_authority_offset = if num_authorities == 0 {
        authorities_offset
    } else {
        // Find end of last authority
        let mut offset = authorities_offset;
        for _ in 0..num_authorities {
            if offset + Position::LEN > current_size {
                break;
            }
            // Parse Position boundary manually to avoid alignment issues
            let position_boundary = u32::from_le_bytes([
                wallet_account_data[offset + 12],
                wallet_account_data[offset + 13],
                wallet_account_data[offset + 14],
                wallet_account_data[offset + 15],
            ]);
            offset = position_boundary as usize;
        }
        offset
    };

    let new_boundary = new_authority_offset + new_authority_size;

    // Create Position structure (Hybrid: includes role_permission)
    let position = Position::new(
        new_authority_type,
        new_authority_data_len,
        num_plugin_refs,
        role_permission,
        new_authority_id,
        new_boundary as u32,
    );

    // Write Position manually to avoid alignment issues
    // Position layout: authority_type (2) + authority_length (2) + num_plugin_refs (2) + role_permission (1) + padding (1) + id (4) + boundary (4) = 16 bytes
    let mut position_bytes = [0u8; Position::LEN];
    position_bytes[0..2].copy_from_slice(&position.authority_type.to_le_bytes());
    position_bytes[2..4].copy_from_slice(&position.authority_length.to_le_bytes());
    position_bytes[4..6].copy_from_slice(&position.num_plugin_refs.to_le_bytes());
    position_bytes[6] = position.role_permission;
    // padding at 7 is already 0
    position_bytes[8..12].copy_from_slice(&position.id.to_le_bytes());
    position_bytes[12..16].copy_from_slice(&position.boundary.to_le_bytes());
    wallet_account_mut_data[new_authority_offset..new_authority_offset + Position::LEN]
        .copy_from_slice(&position_bytes);

    // Write authority data
    let auth_data_offset = new_authority_offset + Position::LEN;
    wallet_account_mut_data[auth_data_offset..auth_data_offset + authority_data.len()]
        .copy_from_slice(authority_data);

    // Write plugin refs (empty initially, but space is allocated)
    let plugin_refs_offset = auth_data_offset + authority_data.len();
    // Plugin refs are zero-initialized (already done by realloc)

    // Update num_authorities
    let new_num_authorities = num_authorities.wrapping_add(1);
    wallet_account.set_num_authorities(wallet_account_mut_data, new_num_authorities)?;

    // CRITICAL: Restore plugin registry to new offset if it was preserved
    if let Some((old_registry_offset, old_num_plugins, old_registry_data)) =
        old_plugin_registry_data
    {
        // Get new registry offset AFTER adding new authority (from wallet_account_mut_data)
        let new_registry_offset = wallet_account
            .plugin_registry_offset(wallet_account_mut_data)
            .map_err(|e| e)?;

        let old_registry_size = old_registry_data.len();
        if new_registry_offset + old_registry_size <= new_account_size_aligned {
            // Restore from preserved data
            wallet_account_mut_data[new_registry_offset..new_registry_offset + old_registry_size]
                .copy_from_slice(&old_registry_data);
        }
    }

    // Ensure rent exemption
    let current_lamports = wallet_account_info.lamports();
    let required_lamports = Rent::get()?.minimum_balance(new_account_size_aligned);
    let lamports_needed = required_lamports.saturating_sub(current_lamports);

    if lamports_needed > 0 {
        Transfer {
            from: payer,
            to: wallet_account_info,
            lamports: lamports_needed,
        }
        .invoke()?;
    }

    Ok(())
}
