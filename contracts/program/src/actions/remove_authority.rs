//! RemoveAuthority instruction handler

use lazorkit_state::{read_position, LazorKitWallet, Position, Transmutable, TransmutableMut};

use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

use crate::error::LazorKitError;

pub fn process_remove_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    acting_role_id: u32,
    target_role_id: u32,
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

    // Cannot remove Owner (role 0)
    if target_role_id == 0 {
        return Err(LazorKitError::Unauthorized.into());
    }

    // Only Owner or Admin can remove authorities
    if acting_role_id != 0 && acting_role_id != 1 {
        return Err(LazorKitError::Unauthorized.into());
    }

    if acting_role_id == target_role_id {
        return Err(LazorKitError::Unauthorized.into());
    }

    let mut config_data = config_account.try_borrow_mut_data()?;
    let role_count = {
        let wallet =
            unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };
        wallet.role_count
    };

    // Find target role and calculate shift
    // Find target role and calculate shift
    let mut target_start: Option<usize> = None;
    let mut target_end: Option<usize> = None;
    let mut total_data_end = 0usize;

    let mut cursor = LazorKitWallet::LEN;

    for _ in 0..role_count {
        if cursor + Position::LEN > config_data.len() {
            break;
        }

        let pos = read_position(&config_data[cursor..])?;

        if pos.id == target_role_id {
            target_start = Some(cursor);
            target_end = Some(pos.boundary as usize);
        }

        total_data_end = pos.boundary as usize;
        cursor = pos.boundary as usize;
    }

    let (target_start, target_end) = match (target_start, target_end) {
        (Some(s), Some(e)) => (s, e),
        _ => {
            msg!("Role {} not found", target_role_id);
            return Err(LazorKitError::AuthorityNotFound.into());
        },
    };

    // Shift data left
    // Shift data left
    let shift_size = target_end - target_start;
    let shift_from = target_end;
    let shift_to = target_start;
    let remaining = total_data_end - shift_from;

    if remaining > 0 {
        config_data.copy_within(shift_from..shift_from + remaining, shift_to);

        // Update boundaries for shifted roles
        let mut cursor = shift_to;
        let end_of_valid_data = shift_to + remaining;

        while cursor < end_of_valid_data {
            if cursor + Position::LEN > config_data.len() {
                break;
            }
            let pos_slice = &mut config_data[cursor..cursor + Position::LEN];
            // Safe because we are within valid data range and alignment is handled by load_mut_unchecked
            let pos = unsafe { Position::load_mut_unchecked(pos_slice)? };

            // Adjust boundary
            pos.boundary = pos
                .boundary
                .checked_sub(shift_size as u32)
                .ok_or(ProgramError::ArithmeticOverflow)?;

            // Move to next role
            cursor = pos.boundary as usize;
        }
    }

    // Update header
    // Update header
    let wallet =
        unsafe { LazorKitWallet::load_mut_unchecked(&mut config_data[..LazorKitWallet::LEN])? };
    wallet.role_count -= 1;

    msg!("Removed authority with role ID {}", target_role_id);

    Ok(())
}
