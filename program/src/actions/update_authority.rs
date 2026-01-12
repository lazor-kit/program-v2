//! Update Authority instruction handler - Pure External Architecture

use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    authority::{authority_type_to_length, AuthorityType},
    plugin_ref::PluginRef,
    position::Position,
    wallet_account::WalletAccount,
    Discriminator, Transmutable,
};
use pinocchio::msg;
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::error::LazorkitError;
use crate::util::permission::check_role_permission_for_authority_management;

/// Arguments for UpdateAuthority instruction (Pure External)
/// Note: instruction discriminator is already parsed in process_action
#[repr(C, align(8))]
#[derive(Debug)]
pub struct UpdateAuthorityArgs {
    pub acting_authority_id: u32, // Authority ID performing this action (for authentication & permission check)
    pub authority_id: u32,        // Authority ID to update
    pub new_authority_type: u16,
    pub new_authority_data_len: u16,
    pub num_plugin_refs: u16, // New number of plugin refs
    pub _padding: [u8; 2],
}

impl UpdateAuthorityArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for UpdateAuthorityArgs {
    const LEN: usize = Self::LEN;
}

/// Updates an authority in the wallet (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. payer (writable, signer) - for rent if account grows
/// 2. system_program
/// 3..N. Additional accounts for authority authentication (signature, etc.)
pub fn update_authority(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
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
    // Parse instruction args
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    if instruction_data.len() < UpdateAuthorityArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse args manually to avoid alignment issues
    let acting_authority_id = u32::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
    ]);
    let authority_id = u32::from_le_bytes([
        instruction_data[4],
        instruction_data[5],
        instruction_data[6],
        instruction_data[7],
    ]);

    let new_authority_type = u16::from_le_bytes([instruction_data[8], instruction_data[9]]);
    let new_authority_data_len = u16::from_le_bytes([instruction_data[10], instruction_data[11]]);
    let num_plugin_refs = u16::from_le_bytes([instruction_data[12], instruction_data[13]]);
    // padding at [14..16] - ignore

    // Parse new authority data
    let authority_data_start = UpdateAuthorityArgs::LEN;
    let authority_data_end = authority_data_start + new_authority_data_len as usize;

    if instruction_data.len() < authority_data_end {
        return Err(ProgramError::InvalidInstructionData);
    }

    let new_authority_data = &instruction_data[authority_data_start..authority_data_end];

    // Validate authority type
    let authority_type = AuthorityType::try_from(new_authority_type)
        .map_err(|_| LazorkitError::InvalidAuthorityType)?;

    // Get acting authority data (for authentication & permission check)
    // Get acting authority data (for authentication & permission check)
    let acting_authority_data = wallet_account
        .get_authority(wallet_account_data, acting_authority_id)?
        .ok_or_else(|| LazorkitError::InvalidAuthorityNotFoundByRoleId)?;

    // Get current authority data (to update)
    // Get current authority data (to update)
    let current_authority_data = wallet_account
        .get_authority(wallet_account_data, authority_id)?
        .ok_or_else(|| LazorkitError::InvalidAuthorityNotFoundByRoleId)?;
    let current_role_perm = current_authority_data
        .position
        .role_permission()
        .map_err(|_| LazorkitError::InvalidRolePermission)?;

    // Pattern: Authenticate → Check Permission → Execute
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

    // HYBRID ARCHITECTURE: Step 2 - Check inline role permission
    // Check if acting authority has permission to manage authorities
    check_role_permission_for_authority_management(&acting_authority_data)?;

    // Find the exact offset of this authority
    let authorities_offset = wallet_account.authorities_offset();
    let num_authorities = wallet_account.num_authorities(wallet_account_data)?;
    let mut authority_offset = authorities_offset;
    let mut found_offset = false;

    for _ in 0..num_authorities {
        if authority_offset + Position::LEN > wallet_account_data.len() {
            break;
        }

        // Parse Position manually
        let position_id = u32::from_le_bytes([
            wallet_account_data[authority_offset + 8],
            wallet_account_data[authority_offset + 9],
            wallet_account_data[authority_offset + 10],
            wallet_account_data[authority_offset + 11],
        ]);
        let position_boundary = u32::from_le_bytes([
            wallet_account_data[authority_offset + 12],
            wallet_account_data[authority_offset + 13],
            wallet_account_data[authority_offset + 14],
            wallet_account_data[authority_offset + 15],
        ]);

        if position_id == authority_id {
            found_offset = true;
            break;
        }

        authority_offset = position_boundary as usize;
    }

    if !found_offset {
        return Err(LazorkitError::InvalidAuthorityNotFoundByRoleId.into());
    }

    // Get old authority size
    let position_boundary = u32::from_le_bytes([
        wallet_account_data[authority_offset + 12],
        wallet_account_data[authority_offset + 13],
        wallet_account_data[authority_offset + 14],
        wallet_account_data[authority_offset + 15],
    ]);
    let old_authority_size = position_boundary as usize - authority_offset;

    // Calculate new authority size
    let plugin_refs_size = num_plugin_refs as usize * PluginRef::LEN;
    let new_authority_size = Position::LEN + new_authority_data_len as usize + plugin_refs_size;

    // Parse plugin refs from instruction_data (if provided)
    // Format: [UpdateAuthorityArgs] + [authority_data] + [plugin_refs]
    let plugin_refs_start = authority_data_end;
    let mut plugin_refs_data: Vec<u8> = Vec::new();

    // Parse plugin refs from instruction_data (if provided)
    // Format: [UpdateAuthorityArgs] + [authority_data] + [plugin_refs]
    let plugin_refs_start = authority_data_end;
    let mut plugin_refs_data = Vec::new();

    // Check if plugin refs are provided
    let required_len = plugin_refs_start + (num_plugin_refs as usize * PluginRef::LEN);
    if instruction_data.len() >= required_len {
        let plugin_refs_end = plugin_refs_start + (num_plugin_refs as usize * PluginRef::LEN);
        plugin_refs_data = instruction_data[plugin_refs_start..plugin_refs_end].to_vec();
    } else {
    }

    // Calculate size difference
    let size_diff = new_authority_size as i32 - old_authority_size as i32;
    let current_size = wallet_account_data.len();
    let new_account_size = (current_size as i32 + size_diff) as usize;

    // Preserve role_permission from current authority BEFORE any resize/modification
    let current_role_permission = wallet_account_data[authority_offset + 6];
    // Preserve role_permission from current authority BEFORE any resize/modification
    let current_role_permission = wallet_account_data[authority_offset + 6];

    // Get mutable access
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };

    if size_diff == 0 {
        // Size unchanged, just update data in place
        // Update Position
        let new_boundary = position_boundary as usize;
        // role_permission already preserved above
        let mut position_bytes = [0u8; Position::LEN];
        position_bytes[0..2].copy_from_slice(&new_authority_type.to_le_bytes());
        position_bytes[2..4].copy_from_slice(&new_authority_data_len.to_le_bytes());
        position_bytes[4..6].copy_from_slice(&num_plugin_refs.to_le_bytes());
        position_bytes[6] = current_role_permission; // Preserve role_permission
        position_bytes[4..6].copy_from_slice(&num_plugin_refs.to_le_bytes());
        position_bytes[6] = current_role_permission; // Preserve role_permission
                                                     // padding at 7 is already 0
        position_bytes[8..12].copy_from_slice(&authority_id.to_le_bytes());
        position_bytes[12..16].copy_from_slice(&(new_boundary as u32).to_le_bytes());

        wallet_account_mut_data[authority_offset..authority_offset + Position::LEN]
            .copy_from_slice(&position_bytes);

        // Write new authority data
        let auth_data_offset = authority_offset + Position::LEN;
        wallet_account_mut_data[auth_data_offset..auth_data_offset + new_authority_data.len()]
            .copy_from_slice(new_authority_data);

        // Write plugin refs
        let plugin_refs_offset = auth_data_offset + new_authority_data.len();
        // Write plugin refs
        let plugin_refs_offset = auth_data_offset + new_authority_data.len();
        if !plugin_refs_data.is_empty() {
            wallet_account_mut_data
                [plugin_refs_offset..plugin_refs_offset + plugin_refs_data.len()]
                .copy_from_slice(&plugin_refs_data);
        } else {
        }

        return Ok(());
    } else if size_diff > 0 {
        // Authority is growing, need to resize account
        let new_account_size_aligned = core::alloc::Layout::from_size_align(new_account_size, 8)
            .map_err(|_| LazorkitError::InvalidAlignment)?
            .pad_to_align()
            .size();

        wallet_account_info.resize(new_account_size_aligned)?;

        // Re-borrow after resize
        let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };

        // Shift data after authority forward to make room
        let data_after_authority = authority_offset + old_authority_size;
        if data_after_authority < current_size {
            let data_to_move_len = current_size - data_after_authority;
            let src_start = data_after_authority;
            let dst_start = authority_offset + new_authority_size;

            // Shift data forward
            wallet_account_mut_data.copy_within(src_start..src_start + data_to_move_len, dst_start);
        }

        // Update boundaries of all authorities after this one
        let mut offset = authorities_offset;
        for _ in 0..num_authorities {
            if offset + Position::LEN > new_account_size {
                break;
            }

            let position_boundary = u32::from_le_bytes([
                wallet_account_mut_data[offset + 12],
                wallet_account_mut_data[offset + 13],
                wallet_account_mut_data[offset + 14],
                wallet_account_mut_data[offset + 15],
            ]);

            // If this authority is after the updated one, adjust boundary
            if offset > authority_offset {
                let new_boundary = position_boundary + (size_diff as u32);
                wallet_account_mut_data[offset + 12..offset + 16]
                    .copy_from_slice(&new_boundary.to_le_bytes());
            }

            offset = position_boundary as usize;
            if offset > authority_offset {
                offset = (offset as i32 + size_diff) as usize;
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
    } else if size_diff < 0 {
        // Authority is shrinking, compact data
        let new_account_size_aligned = core::alloc::Layout::from_size_align(new_account_size, 8)
            .map_err(|_| LazorkitError::InvalidAlignment)?
            .pad_to_align()
            .size();

        // Update boundaries first
        let mut offset = authorities_offset;
        for _ in 0..num_authorities {
            if offset + Position::LEN > current_size {
                break;
            }

            let position_boundary = u32::from_le_bytes([
                wallet_account_mut_data[offset + 12],
                wallet_account_mut_data[offset + 13],
                wallet_account_mut_data[offset + 14],
                wallet_account_mut_data[offset + 15],
            ]);

            // If this authority is after the updated one, adjust boundary
            if offset > authority_offset {
                let new_boundary = position_boundary.saturating_sub((-size_diff) as u32);
                wallet_account_mut_data[offset + 12..offset + 16]
                    .copy_from_slice(&new_boundary.to_le_bytes());
            }

            offset = position_boundary as usize;
        }

        // Shift data backward to compact
        let data_after_authority = authority_offset + old_authority_size;
        if data_after_authority < current_size {
            let data_to_move_len = current_size - data_after_authority;
            let src_start = data_after_authority;
            let dst_start = authority_offset + new_authority_size;

            // Shift data backward
            wallet_account_mut_data.copy_within(src_start..src_start + data_to_move_len, dst_start);
        }

        // Resize account
        wallet_account_info.resize(new_account_size_aligned)?;

        // Refund excess lamports
        let current_lamports = wallet_account_info.lamports();
        let required_lamports = Rent::get()?.minimum_balance(new_account_size_aligned);
        let excess_lamports = current_lamports.saturating_sub(required_lamports);

        if excess_lamports > 0 {
            Transfer {
                from: wallet_account_info,
                to: payer,
                lamports: excess_lamports,
            }
            .invoke()?;
        }
    }

    // Re-borrow after potential resize
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };

    // Update Position
    let new_boundary = authority_offset + new_authority_size;
    // role_permission already preserved above (before resize)
    let mut position_bytes = [0u8; Position::LEN];
    position_bytes[0..2].copy_from_slice(&new_authority_type.to_le_bytes());
    position_bytes[2..4].copy_from_slice(&new_authority_data_len.to_le_bytes());
    position_bytes[4..6].copy_from_slice(&num_plugin_refs.to_le_bytes());
    position_bytes[6] = current_role_permission; // Preserve role_permission
                                                 // padding at 7 is already 0
    position_bytes[8..12].copy_from_slice(&authority_id.to_le_bytes());
    position_bytes[12..16].copy_from_slice(&(new_boundary as u32).to_le_bytes());

    wallet_account_mut_data[authority_offset..authority_offset + Position::LEN]
        .copy_from_slice(&position_bytes);

    // Write new authority data
    let auth_data_offset = authority_offset + Position::LEN;
    wallet_account_mut_data[auth_data_offset..auth_data_offset + new_authority_data.len()]
        .copy_from_slice(new_authority_data);

    // Write plugin refs
    let plugin_refs_offset = auth_data_offset + new_authority_data.len();
    if !plugin_refs_data.is_empty() {
        wallet_account_mut_data[plugin_refs_offset..plugin_refs_offset + plugin_refs_data.len()]
            .copy_from_slice(&plugin_refs_data);
    } else if num_plugin_refs > 0 {
        // Zero-initialize plugin refs space if no data provided
        // (space is already zero-initialized by resize)
    }

    Ok(())
}
