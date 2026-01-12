//! Create Session instruction handler - Pure External Architecture

use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    authority::AuthorityType, plugin::PluginEntry, plugin_ref::PluginRef, position::Position,
    wallet_account::WalletAccount, Discriminator, Transmutable,
};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::error::LazorkitError;
use crate::util::plugin::check_plugin_permission_for_instruction_data;

/// Arguments for CreateSession instruction (Pure External)
/// Note: instruction discriminator is already parsed in process_action
#[repr(C, align(8))]
#[derive(Debug)]
pub struct CreateSessionArgs {
    pub authority_id: u32, // Authority ID to create session for
    pub session_duration: u64,
    pub session_key: [u8; 32],
}

impl CreateSessionArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for CreateSessionArgs {
    const LEN: usize = Self::LEN;
}

/// Creates a new authentication session for a wallet authority (Pure External architecture).
///
/// This converts a standard authority to a session-based authority by updating
/// the authority data in WalletAccount.
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1..N. Additional accounts for authority authentication (signature, etc.)
pub fn create_session(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    if accounts.len() < 1 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }

    let wallet_account_info = &accounts[0];

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
    if instruction_data.len() < CreateSessionArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse args manually to avoid alignment issues
    // CreateSessionArgs layout with #[repr(C, align(8))]:
    // - authority_id: u32 at offset 0 (4 bytes)
    // - padding: [u8; 4] at offset 4 (4 bytes)
    // - session_duration: u64 at offset 8 (8 bytes)
    // - session_key: [u8; 32] at offset 16 (32 bytes)
    // - padding: [u8; 8] at offset 48 (8 bytes)
    // Total: 56 bytes
    let authority_id = u32::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
    ]);
    // Skip padding at offset 4-7
    let session_duration = u64::from_le_bytes([
        instruction_data[8],
        instruction_data[9],
        instruction_data[10],
        instruction_data[11],
        instruction_data[12],
        instruction_data[13],
        instruction_data[14],
        instruction_data[15],
    ]);
    let mut session_key = [0u8; 32];
    session_key.copy_from_slice(&instruction_data[16..48]);

    // Get authority data
    let authority_data = wallet_account
        .get_authority(wallet_account_data, authority_id)?
        .ok_or(LazorkitError::InvalidAuthorityNotFoundByRoleId)?;

    // Parse authority type
    let authority_type = AuthorityType::try_from(authority_data.position.authority_type)
        .map_err(|_| LazorkitError::InvalidAuthorityType)?;

    // Check if authority is already session-based
    // Session-based authority types are: Ed25519Session (2), Secp256k1Session (4), Secp256r1Session (6), ProgramExecSession (8)
    if matches!(authority_data.position.authority_type, 2 | 4 | 6 | 8) {
        return Err(LazorkitError::InvalidAuthorityType.into());
    }

    // Pattern: Authenticate → CPI plugin check permission → Execute
    // Step 1: Authenticate authority (verify signature)
    // Accounts order: [0] wallet_account, [1] payer, [2] system_program, [3] authority_payload, [4] acting_authority
    let authority_payload = accounts
        .get(3)
        .map(|acc| unsafe { acc.borrow_data_unchecked() });
    crate::util::authenticate::authenticate_authority(
        &authority_data,
        accounts,
        authority_payload,
        Some(instruction_data),
    )?;

    // Step 2: CPI to plugins to check permission
    // Plugin decides if authority has permission to create session
    let all_plugins = wallet_account.get_plugins(wallet_account_data)?;

    // Get enabled plugin refs for authority (sorted by priority)
    let mut enabled_refs: Vec<&PluginRef> = authority_data
        .plugin_refs
        .iter()
        .filter(|r| r.is_enabled())
        .collect();
    enabled_refs.sort_by_key(|r| r.priority);

    // CPI to each enabled plugin to check permission
    for plugin_ref in &enabled_refs {
        let plugin = &all_plugins[plugin_ref.plugin_index as usize];

        check_plugin_permission_for_instruction_data(
            plugin,
            &authority_data,
            instruction_data,
            accounts,
            wallet_account_info,
            None, // No wallet_vault for create_session
        )?;
    }

    // Step 3: Execute action (create session)

    // Get clock for session expiration
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    let expiration_slot = current_slot.saturating_add(session_duration);

    // Find authority offset in account data
    let authorities_offset = wallet_account.authorities_offset();
    let num_authorities = wallet_account.num_authorities(wallet_account_data)?;
    let mut authority_offset = authorities_offset;
    let mut found_offset = false;

    #[allow(unused_assignments)]
    let mut position_boundary = 0u32;

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

        position_boundary = u32::from_le_bytes([
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

    // Calculate new session-based authority size
    // Session-based authorities have additional fields based on authority type:
    // - Ed25519Session: session_key (32) + max_session_length (8) + expiration (8) = 48 bytes
    // - Secp256k1Session/Secp256r1Session: padding (3) + signature_odometer (4) + session_key (32) + max_session_age (8) + expiration (8) = 55 bytes
    let old_authority_data_len = authority_data.position.authority_length as usize;
    let session_data_size = match authority_data.position.authority_type {
        1 => 48, // Ed25519 -> Ed25519Session
        3 => 55, // Secp256k1 -> Secp256k1Session
        5 => 55, // Secp256r1 -> Secp256r1Session
        7 => 48, // ProgramExec -> ProgramExecSession (similar to Ed25519)
        _ => return Err(LazorkitError::InvalidAuthorityType.into()),
    };
    let new_authority_data_len = old_authority_data_len + session_data_size;

    // Calculate size difference
    let size_diff = session_data_size;
    let current_size = wallet_account_data.len();
    let new_account_size = current_size + size_diff;

    // Get mutable access
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };

    // Resize account to accommodate session data
    let new_account_size_aligned = core::alloc::Layout::from_size_align(new_account_size, 8)
        .map_err(|_| LazorkitError::InvalidAlignment)?
        .pad_to_align()
        .size();

    wallet_account_info.resize(new_account_size_aligned)?;

    // Re-borrow after resize
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };

    // Get old boundary
    let position_boundary = u32::from_le_bytes([
        wallet_account_mut_data[authority_offset + 12],
        wallet_account_mut_data[authority_offset + 13],
        wallet_account_mut_data[authority_offset + 14],
        wallet_account_mut_data[authority_offset + 15],
    ]);

    // Shift data after authority forward to make room for session data
    let data_after_authority = position_boundary as usize;
    if data_after_authority < current_size {
        let data_to_move_len = current_size - data_after_authority;
        let src_start = data_after_authority;
        let dst_start = data_after_authority + size_diff;

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
            offset = offset + size_diff;
        }
    }

    // Update Position: change authority_type to session-based and update length
    let new_authority_type = match authority_data.position.authority_type {
        1 => 2u16, // Ed25519 -> Ed25519Session
        3 => 4u16, // Secp256k1 -> Secp256k1Session
        5 => 6u16, // Secp256r1 -> Secp256r1Session
        7 => 8u16, // ProgramExec -> ProgramExecSession
        _ => return Err(LazorkitError::InvalidAuthorityType.into()),
    };

    let new_boundary = position_boundary as usize + size_diff;

    // Update Position
    wallet_account_mut_data[authority_offset..authority_offset + 2]
        .copy_from_slice(&new_authority_type.to_le_bytes());
    wallet_account_mut_data[authority_offset + 2..authority_offset + 4]
        .copy_from_slice(&(new_authority_data_len as u16).to_le_bytes());
    wallet_account_mut_data[authority_offset + 12..authority_offset + 16]
        .copy_from_slice(&(new_boundary as u32).to_le_bytes());

    // Append session data based on authority type
    let session_data_offset = position_boundary as usize;
    match authority_data.position.authority_type {
        1 => {
            // Ed25519Session: session_key (32) + max_session_length (8) + expiration (8)
            wallet_account_mut_data[session_data_offset..session_data_offset + 32]
                .copy_from_slice(&session_key);
            wallet_account_mut_data[session_data_offset + 32..session_data_offset + 40]
                .copy_from_slice(&session_duration.to_le_bytes()); // max_session_length
            wallet_account_mut_data[session_data_offset + 40..session_data_offset + 48]
                .copy_from_slice(&expiration_slot.to_le_bytes()); // current_session_expiration
        }
        3 | 5 => {
            // Secp256k1Session/Secp256r1Session: padding (3) + signature_odometer (4) + session_key (32) + max_session_age (8) + expiration (8)
            // padding (3 bytes) - already zero-initialized
            wallet_account_mut_data[session_data_offset + 3..session_data_offset + 7]
                .copy_from_slice(&0u32.to_le_bytes()); // signature_odometer = 0
            wallet_account_mut_data[session_data_offset + 7..session_data_offset + 39]
                .copy_from_slice(&session_key);
            wallet_account_mut_data[session_data_offset + 39..session_data_offset + 47]
                .copy_from_slice(&session_duration.to_le_bytes()); // max_session_age
            wallet_account_mut_data[session_data_offset + 47..session_data_offset + 55]
                .copy_from_slice(&expiration_slot.to_le_bytes()); // current_session_expiration
        }
        7 => {
            // ProgramExecSession: similar to Ed25519Session
            wallet_account_mut_data[session_data_offset..session_data_offset + 32]
                .copy_from_slice(&session_key);
            wallet_account_mut_data[session_data_offset + 32..session_data_offset + 40]
                .copy_from_slice(&session_duration.to_le_bytes()); // max_session_length
            wallet_account_mut_data[session_data_offset + 40..session_data_offset + 48]
                .copy_from_slice(&expiration_slot.to_le_bytes()); // current_session_expiration
        }
        _ => return Err(LazorkitError::InvalidAuthorityType.into()),
    }

    // Ensure rent exemption
    use pinocchio::sysvars::rent::Rent;
    use pinocchio_system::instructions::Transfer;
    let current_lamports = wallet_account_info.lamports();
    let required_lamports = Rent::get()?.minimum_balance(new_account_size_aligned);
    let lamports_needed = required_lamports.saturating_sub(current_lamports);

    if lamports_needed > 0 {
        // Note: In Pure External, payer should be passed as account[1]
        // For now, we'll skip rent transfer if payer is not provided
        if accounts.len() > 1 {
            let payer = &accounts[1];
            Transfer {
                from: payer,
                to: wallet_account_info,
                lamports: lamports_needed,
            }
            .invoke()?;
        }
    }

    Ok(())
}
