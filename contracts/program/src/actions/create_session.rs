//! CreateSession instruction handler

use lazorkit_state::authority::ed25519::Ed25519SessionAuthority;
use lazorkit_state::authority::secp256r1::Secp256r1SessionAuthority;
use lazorkit_state::{
    read_position, AuthorityInfo, AuthorityType, LazorKitWallet, Position, RoleIterator,
    Transmutable, TransmutableMut,
};
use pinocchio::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::error::LazorKitError;

pub fn process_create_session(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    role_id: u32,
    session_key: [u8; 32],
    duration: u64,
    authorization_data: &[u8],
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

    // Release immutable borrow and get mutable borrow for update
    // We scope the search to avoid borrow conflicts
    let found = {
        let config_data = config_account.try_borrow_data()?;
        let wallet =
            unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };
        let mut found = false;

        // Iterator now returns Result, so we need to handle it
        for item in RoleIterator::new(&config_data, wallet.role_count, LazorKitWallet::LEN) {
            let (pos, auth_data) = item?; // Unwrap Result
            if pos.id == role_id {
                found = true;
                // Check if it has Secp256k1 or Secp256r1 authority
                let auth_type = AuthorityType::try_from(pos.authority_type)?;
                match auth_type {
                    AuthorityType::Ed25519Session => {
                        // Ed25519Session: Verify master key signature via simple signer check
                        if auth_data.len() < 32 {
                            return Err(ProgramError::InvalidAccountData);
                        }
                        let master_pubkey = &auth_data[0..32];
                        let mut is_authorized = false;
                        for acc in accounts {
                            if acc.is_signer() && acc.key().as_ref() == master_pubkey {
                                is_authorized = true;
                                break;
                            }
                        }
                        if !is_authorized {
                            msg!("Missing signature from Ed25519 master key");
                            return Err(ProgramError::MissingRequiredSignature);
                        }
                        // found already set to true at line 59
                    },
                    AuthorityType::Secp256r1Session => {
                        // Secp256r1Session: Will authenticate via full Secp256r1 flow later
                        // This includes counter-based replay protection + precompile verification
                        // See lines 147-156 for the actual authentication
                        // found already set to true at line 59
                    },
                    _ => {
                        msg!("Authority type {:?} does not support sessions", auth_type);
                        return Err(LazorKitError::InvalidInstruction.into());
                    },
                }
                break;
            }
        }
        found
    };

    // Explicitly do nothing here, just ensuring previous block is closed.

    if !found {
        msg!("Role {} not found or doesn't support sessions", role_id);
        return Err(LazorKitError::AuthorityNotFound.into());
    }

    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    let mut config_data = config_account.try_borrow_mut_data()?;
    let role_count =
        unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])?.role_count };

    // Manual loop for update (Zero-Copy in-place modification)
    let mut cursor = LazorKitWallet::LEN;

    // We already verified it exists.
    for _ in 0..role_count {
        if cursor + Position::LEN > config_data.len() {
            break;
        }

        // We need to read position. read_position works on slice.
        // It returns Position.
        let pos = match read_position(&config_data[cursor..]) {
            Ok(p) => *p,
            Err(_) => break,
        };

        if pos.id == role_id {
            let auth_start = cursor + Position::LEN;
            let auth_end = auth_start + pos.authority_length as usize;

            if auth_end > config_data.len() {
                break;
            }

            let auth_slice = &mut config_data[auth_start..auth_end];

            // Construct data payload for authentication (Role + Session Params)
            let mut data_payload = [0u8; 4 + 32 + 8];
            data_payload[0..4].copy_from_slice(&role_id.to_le_bytes());
            data_payload[4..36].copy_from_slice(&session_key);
            data_payload[36..44].copy_from_slice(&duration.to_le_bytes());

            // Update logic (Zero-Copy)
            let auth_type = AuthorityType::try_from(pos.authority_type)?;
            match auth_type {
                AuthorityType::Ed25519Session => {
                    let auth = unsafe { Ed25519SessionAuthority::load_mut_unchecked(auth_slice)? };
                    auth.start_session(session_key, current_slot, duration)?;
                    msg!("Ed25519 session created");
                },
                AuthorityType::Secp256r1Session => {
                    let auth =
                        unsafe { Secp256r1SessionAuthority::load_mut_unchecked(auth_slice)? };

                    // Verify Signature
                    auth.authenticate(accounts, authorization_data, &data_payload, current_slot)?;

                    auth.start_session(session_key, current_slot, duration)?;
                    msg!("Secp256r1 session created");
                },
                _ => return Err(LazorKitError::InvalidInstruction.into()),
            }
            break;
        }

        cursor = pos.boundary as usize;
    }

    Ok(())
}
