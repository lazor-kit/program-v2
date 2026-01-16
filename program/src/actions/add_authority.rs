//! AddAuthority instruction handler
//!
//! Adds a new authority/role to the wallet.

use lazorkit_interface::{VerifyInstruction, INSTRUCTION_VERIFY};
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

use crate::actions::verify_policy_registry;
use crate::error::LazorKitError;

pub fn process_add_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    acting_role_id: u32,
    authority_type: u16,
    authority_data: Vec<u8>,
    policies_config: Vec<u8>,
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
        let mut config_data = config_account.try_borrow_mut_data()?;
        let (wallet_header, roles_data) = config_data.split_at_mut(LazorKitWallet::LEN);
        let wallet = unsafe { LazorKitWallet::load_unchecked(wallet_header)? };

        let mut current_offset = 0;
        let mut authenticated = false;

        for _ in 0..wallet.role_count {
            if current_offset + Position::LEN > roles_data.len() {
                return Err(ProgramError::InvalidAccountData);
            }
            let pos = unsafe {
                Position::load_unchecked(
                    &roles_data[current_offset..current_offset + Position::LEN],
                )?
            };

            if pos.id == acting_role_id {
                let auth_start = current_offset + Position::LEN;
                let auth_end = auth_start + pos.authority_length as usize;

                if auth_end > roles_data.len() {
                    return Err(ProgramError::InvalidAccountData);
                }

                let auth_type_enum = lazorkit_state::AuthorityType::try_from(pos.authority_type)?;

                #[derive(borsh::BorshSerialize)]
                struct AddAuthPayload<'a> {
                    acting_role_id: u32,
                    authority_type: u16,
                    authority_data: &'a [u8],
                    policies_config: &'a [u8],
                }

                let payload_struct = AddAuthPayload {
                    acting_role_id,
                    authority_type,
                    authority_data: &authority_data,
                    policies_config: &policies_config,
                };
                let data_payload = borsh::to_vec(&payload_struct)
                    .map_err(|_| ProgramError::InvalidInstructionData)?;

                match auth_type_enum {
                    lazorkit_state::AuthorityType::Ed25519 => {
                        let auth = unsafe {
                            lazorkit_state::Ed25519Authority::load_mut_unchecked(
                                &mut roles_data[auth_start..auth_end],
                            )?
                        };
                        auth.authenticate(accounts, &authorization_data, &data_payload, 0)?;
                    },
                    lazorkit_state::AuthorityType::Secp256k1 => {
                        let clock = pinocchio::sysvars::clock::Clock::get()?;
                        let auth = unsafe {
                            lazorkit_state::Secp256k1Authority::load_mut_unchecked(
                                &mut roles_data[auth_start..auth_end],
                            )?
                        };
                        auth.authenticate(
                            accounts,
                            &authorization_data,
                            &data_payload,
                            clock.slot,
                        )?;
                    },
                    lazorkit_state::AuthorityType::Secp256r1 => {
                        let clock = pinocchio::sysvars::clock::Clock::get()?;
                        let auth = unsafe {
                            lazorkit_state::Secp256r1Authority::load_mut_unchecked(
                                &mut roles_data[auth_start..auth_end],
                            )?
                        };
                        auth.authenticate(
                            accounts,
                            &authorization_data,
                            &data_payload,
                            clock.slot,
                        )?;
                    },
                    _ => return Err(ProgramError::InvalidInstructionData),
                }

                authenticated = true;
                break;
            }
            current_offset = pos.boundary as usize;
        }

        if !authenticated {
            return Err(LazorKitError::Unauthorized.into());
        }
    }

    // Permission check
    // Allow Owner (0) and Admin (1)
    if acting_role_id != 0 && acting_role_id != 1 {
        msg!("Only Owner or Admin can add authorities");
        return Err(LazorKitError::Unauthorized.into());
    }

    // 2. Validate New Role Params
    let auth_type = lazorkit_state::AuthorityType::try_from(authority_type)?;
    let expected_len = authority_type_to_length(&auth_type)?;
    if authority_data.len() != expected_len {
        return Err(ProgramError::InvalidInstructionData);
    }

    let policies_len = policies_config.len();
    let num_policies = lazorkit_state::policy::parse_policies(&policies_config).count() as u16;

    // Registry Verification
    if num_policies > 0 {
        let registry_accounts = &accounts[3..];
        for policy in lazorkit_state::policy::parse_policies(&policies_config) {
            let p = policy.map_err(|_| ProgramError::InvalidInstructionData)?;
            let pid = Pubkey::from(p.header.program_id);
            verify_policy_registry(program_id, &pid, registry_accounts)?;
        }
    }

    // 3. Resize and Append
    let new_role_id = {
        let config_data = config_account.try_borrow_data()?;
        let wallet =
            unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };
        wallet.role_counter
    };

    let position_len = Position::LEN;
    let required_space = position_len + expected_len + policies_len;
    let new_len = config_account.data_len() + required_space;

    reallocate_account(config_account, payer_account, new_len)?;

    let mut config_data = config_account.try_borrow_mut_data()?;
    let (wallet_slice, remainder_slice) = config_data.split_at_mut(LazorKitWallet::LEN);

    let mut wallet = unsafe { LazorKitWallet::load_mut_unchecked(wallet_slice)? };

    let total_len = LazorKitWallet::LEN + remainder_slice.len();
    let write_offset_abs = total_len - required_space;
    let write_offset_rel = write_offset_abs - LazorKitWallet::LEN;

    let new_pos = Position {
        authority_type,
        authority_length: expected_len as u16,
        num_policies,
        padding: 0,
        id: new_role_id,
        boundary: (total_len as u32),
    };

    let pos_slice = &mut remainder_slice[write_offset_rel..write_offset_rel + Position::LEN];
    let pos_ref = unsafe { Position::load_mut_unchecked(pos_slice)? };
    *pos_ref = new_pos;

    let auth_offset_rel = write_offset_rel + Position::LEN;
    remainder_slice[auth_offset_rel..auth_offset_rel + expected_len]
        .copy_from_slice(&authority_data);

    let policies_offset_rel = auth_offset_rel + expected_len;
    if policies_len > 0 {
        remainder_slice[policies_offset_rel..policies_offset_rel + policies_len]
            .copy_from_slice(&policies_config);
    }

    wallet.role_counter += 1;
    wallet.role_count += 1;

    msg!(
        "Added role {} with type {:?} and {} policies",
        new_role_id,
        auth_type,
        num_policies
    );

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
