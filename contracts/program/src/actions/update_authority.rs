//! UpdateAuthority instruction handler
//!
//! Updates an existing authority's data (key rotation, session limits, etc.)

use lazorkit_state::{
    authority::{authority_type_to_length, Authority, AuthorityType},
    read_position, LazorKitWallet, Position, Transmutable, TransmutableMut,
};
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

use crate::actions::{authenticate_role, find_role};
use crate::error::LazorKitError;

pub fn process_update_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    acting_role_id: u32,
    target_role_id: u32,
    new_authority_data: Vec<u8>,
    authorization_data: Vec<u8>,
) -> ProgramResult {
    let mut account_info_iter = accounts.iter();
    let config_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let payer_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let _system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !payer_account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if config_account.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Cannot update Owner (role 0)
    if target_role_id == 0 {
        return Err(LazorKitError::Unauthorized.into());
    }

    // 1. Authenticate acting role
    {
        #[derive(borsh::BorshSerialize)]
        struct UpdateAuthPayload<'a> {
            acting_role_id: u32,
            target_role_id: u32,
            new_authority_data: &'a [u8],
        }

        let payload_struct = UpdateAuthPayload {
            acting_role_id,
            target_role_id,
            new_authority_data: &new_authority_data,
        };
        let data_payload =
            borsh::to_vec(&payload_struct).map_err(|_| ProgramError::InvalidInstructionData)?;

        authenticate_role(
            config_account,
            acting_role_id,
            accounts,
            &authorization_data,
            &data_payload,
        )?;
    }

    // Permission check: Only Owner (0) or Admin (1) can update authorities
    if acting_role_id != 0 && acting_role_id != 1 {
        msg!("Only Owner or Admin can update authorities");
        return Err(LazorKitError::Unauthorized.into());
    }

    // Cannot update self
    if acting_role_id == target_role_id {
        return Err(LazorKitError::Unauthorized.into());
    }

    // 2. Find target role and validate
    let mut config_data = config_account.try_borrow_mut_data()?;
    let wallet = unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };
    let mut target_offset: Option<usize> = None;
    let mut target_pos: Option<Position> = None;

    let mut cursor = LazorKitWallet::LEN;
    for _ in 0..wallet.role_count {
        if cursor + Position::LEN > config_data.len() {
            break;
        }

        let pos = read_position(&config_data[cursor..])?;
        if pos.id == target_role_id {
            target_offset = Some(cursor);
            target_pos = Some(*pos);
            break;
        }

        cursor = pos.boundary as usize;
    }

    let (target_offset, target_pos) = match (target_offset, target_pos) {
        (Some(offset), Some(pos)) => (offset, pos),
        _ => {
            msg!("Role {} not found", target_role_id);
            return Err(LazorKitError::AuthorityNotFound.into());
        },
    };

    // 3. Validate new authority data matches the existing type
    let auth_type = AuthorityType::try_from(target_pos.authority_type)?;
    let expected_len = authority_type_to_length(&auth_type)?;

    // Validate data length based on authority type
    let valid_data = match auth_type {
        AuthorityType::Ed25519 => new_authority_data.len() == 32,
        AuthorityType::Ed25519Session => new_authority_data.len() == 72, // 32+32+8
        AuthorityType::Secp256r1 => new_authority_data.len() == 33,
        AuthorityType::Secp256r1Session => new_authority_data.len() == 73, // 33+32+8
        _ => false,
    };

    if !valid_data {
        msg!(
            "Invalid authority data length for type {:?}: expected creation data, got {}",
            auth_type,
            new_authority_data.len()
        );
        return Err(LazorKitError::InvalidInstruction.into());
    }

    // 4. Update authority data in place (zero-copy)
    let auth_start = target_offset + Position::LEN;
    let auth_end = auth_start + expected_len;

    if auth_end > config_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }

    let auth_slice = &mut config_data[auth_start..auth_end];

    // Use Authority::set_into_bytes to update the authority data
    match auth_type {
        AuthorityType::Ed25519 => {
            lazorkit_state::Ed25519Authority::set_into_bytes(&new_authority_data, auth_slice)?;
        },
        AuthorityType::Ed25519Session => {
            lazorkit_state::Ed25519SessionAuthority::set_into_bytes(
                &new_authority_data,
                auth_slice,
            )?;
        },
        AuthorityType::Secp256r1 => {
            lazorkit_state::Secp256r1Authority::set_into_bytes(&new_authority_data, auth_slice)?;
        },
        AuthorityType::Secp256r1Session => {
            lazorkit_state::Secp256r1SessionAuthority::set_into_bytes(
                &new_authority_data,
                auth_slice,
            )?;
        },
        _ => return Err(ProgramError::InvalidInstructionData),
    }

    msg!("Updated authority for role ID {}", target_role_id);
    Ok(())
}
