//! AddAuthority instruction handler
//!
//! Adds a new authority/role to the wallet.

use lazorkit_state::{
    authority::authority_type_to_length, authority::AuthorityInfo, IntoBytes, LazorKitWallet,
    Position, Transmutable, TransmutableMut,
};
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::actions::{authenticate_role, find_role, require_admin_or_owner};
use crate::error::LazorKitError;

pub fn process_add_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    acting_role_id: u32,
    authority_type: u16,
    authority_data: Vec<u8>,
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

    // 1. Authenticate
    {
        #[derive(borsh::BorshSerialize)]
        struct AddAuthPayload<'a> {
            acting_role_id: u32,
            authority_type: u16,
            authority_data: &'a [u8],
        }

        let payload_struct = AddAuthPayload {
            acting_role_id,
            authority_type,
            authority_data: &authority_data,
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

    // Permission check: Only Owner (0) or Admin (1) can add authorities
    // Find the acting role's position to check permissions
    let (acting_position, _offset) = {
        let config_data = config_account.try_borrow_data()?;
        find_role(&config_data, acting_role_id)?
    };
    require_admin_or_owner(&acting_position)?;

    // 2. Validate New Role Params
    let auth_type = lazorkit_state::AuthorityType::try_from(authority_type)?;
    let expected_len = authority_type_to_length(&auth_type)?;

    // Validate authority data length to prevent buffer overflow/underflow
    if authority_data.len() != expected_len {
        msg!(
            "Authority data length mismatch: expected {} bytes, got {} bytes",
            expected_len,
            authority_data.len()
        );
        return Err(crate::error::LazorKitError::InvalidAuthorityData.into());
    }

    // 3. Resize and Append
    let required_space = Position::LEN + expected_len;
    let new_len = config_account.data_len() + required_space;

    reallocate_account(config_account, payer_account, new_len)?;

    let config_data = unsafe { config_account.borrow_mut_data_unchecked() };

    // Determine role type: First non-owner role gets Admin (1), subsequent roles get Spender (2)
    let role_type = {
        let wallet = unsafe {
            lazorkit_state::LazorKitWallet::load_unchecked(
                &config_data[..lazorkit_state::LazorKitWallet::LEN],
            )?
        };

        // Validate discriminator
        if !wallet.is_valid() {
            return Err(ProgramError::InvalidAccountData);
        }

        if wallet.role_count == 1 {
            1 // First non-owner role = Admin
        } else {
            2 // Subsequent roles = Spender
        }
    };

    let mut builder = lazorkit_state::LazorKitBuilder::new_from_bytes(config_data)?;

    // add_role handles set_into_bytes and updating all metadata (bump, counters, boundaries)
    builder.add_role(auth_type, &authority_data, role_type)?;

    Ok(())
}

fn reallocate_account(
    account: &AccountInfo,
    payer: &AccountInfo,
    new_size: usize,
) -> ProgramResult {
    let rent = Rent::get()?;
    let new_minimum_balance = rent.minimum_balance(new_size);
    let lamports_diff = new_minimum_balance.saturating_sub(account.lamports());

    if lamports_diff > 0 {
        Transfer {
            from: payer,
            to: account,
            lamports: lamports_diff,
        }
        .invoke()?;
    }

    account.resize(new_size)?;
    Ok(())
}
