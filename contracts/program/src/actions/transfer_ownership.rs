//! TransferOwnership instruction handler

use lazorkit_state::{
    authority::{
        authority_type_to_length,
        ed25519::{Ed25519Authority, Ed25519SessionAuthority},
        secp256r1::{Secp256r1Authority, Secp256r1SessionAuthority},
        AuthorityInfo, AuthorityType,
    },
    read_position, LazorKitWallet, Position, Transmutable, TransmutableMut,
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

pub fn process_transfer_ownership(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_owner_authority_type: u16,
    new_owner_authority_data: Vec<u8>,
    auth_payload: Vec<u8>,
) -> ProgramResult {
    let mut account_info_iter = accounts.iter();
    let config_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let owner_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !owner_account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if config_account.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    let mut config_data = config_account.try_borrow_mut_data()?;
    let _wallet = unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };

    // Validate new authority type
    let new_auth_type = AuthorityType::try_from(new_owner_authority_type)?;
    let new_auth_len = authority_type_to_length(&new_auth_type)?;

    if new_owner_authority_data.len() != new_auth_len {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Find Role 0 (Owner)
    let role_buffer = &mut config_data[LazorKitWallet::LEN..];

    // Read current owner position
    let current_pos = read_position(role_buffer)?;
    if current_pos.id != 0 {
        msg!("First role is not Owner");
        return Err(LazorKitError::InvalidWalletAccount.into());
    }

    // Copy values from current_pos to avoid borrow checker conflicts
    let current_auth_len = current_pos.authority_length as usize;
    let current_auth_type = AuthorityType::try_from(current_pos.authority_type)?;
    let current_boundary = current_pos.boundary;

    // Get current slot for session expiration checks
    // Get current slot for session expiration checks
    let slot = Clock::get()?.slot;

    // Authenticate using the same pattern as Execute
    // IMPORTANT: Scope this block to drop the mutable borrow before accessing role_buffer again
    {
        let authority_data_slice =
            &mut role_buffer[Position::LEN..Position::LEN + current_auth_len];

        // Define auth payloads based on type
        // Ed25519: authority_payload = [index], data_payload = [ignored]
        // Secp256r1: authority_payload = [sig_struct], data_payload = [new_owner_data]
        let (auth_p1, auth_p2) = match current_auth_type {
            AuthorityType::Ed25519 | AuthorityType::Ed25519Session => {
                if !auth_payload.is_empty() {
                    auth_payload.split_at(1)
                } else {
                    (&[] as &[u8], &[] as &[u8])
                }
            },
            AuthorityType::Secp256r1 | AuthorityType::Secp256r1Session => {
                (auth_payload.as_slice(), new_owner_authority_data.as_slice())
            },
            _ => {
                if !auth_payload.is_empty() {
                    auth_payload.split_at(1)
                } else {
                    (&[] as &[u8], &[] as &[u8])
                }
            },
        };

        // Macro to simplify auth calls (reused from execute.rs)
        macro_rules! authenticate_auth {
            ($auth_type:ty) => {{
                let mut auth = unsafe { <$auth_type>::load_mut_unchecked(authority_data_slice) }
                    .map_err(|_| ProgramError::InvalidAccountData)?;
                if auth.session_based() {
                    auth.authenticate_session(accounts, auth_p1, auth_p2, slot)?;
                } else {
                    auth.authenticate(accounts, auth_p1, auth_p2, slot)?;
                }
            }};
        }

        match current_auth_type {
            AuthorityType::Ed25519 => authenticate_auth!(Ed25519Authority),
            AuthorityType::Ed25519Session => authenticate_auth!(Ed25519SessionAuthority),
            AuthorityType::Secp256r1 => authenticate_auth!(Secp256r1Authority),
            AuthorityType::Secp256r1Session => authenticate_auth!(Secp256r1SessionAuthority),
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    }
    // Mutable borrow of authority_data_slice has been dropped here

    // IMPORTANT: Authority Size Change Restriction
    //
    // We do not support changing authority sizes during ownership transfer because it would require:
    // 1. Shifting all subsequent role data in the account buffer (expensive compute)
    // 2. Potential account reallocation if the new authority is larger
    // 3. Complex error recovery if reallocation fails mid-transfer
    // 4. Risk of wallet corruption if the operation is interrupted
    //
    // Supported transfers (same-size, safe in-place replacement):
    // - Ed25519 (32) → Ed25519 (32) ✅
    // - Secp256r1 (40) → Secp256r1 (40) ✅
    //
    // NOT supported (different sizes, requires data migration):
    // - Ed25519 (32) → Ed25519Session (80) ❌
    // - Ed25519 (32) → Secp256r1 (40) ❌
    // - Secp256r1 (40) → Secp256r1Session (88) ❌
    //
    // Workaround for different authority types:
    // 1. Create a new wallet with the desired owner authority type
    // 2. Transfer all assets from old wallet to new wallet
    // 3. Deprecate the old wallet
    if new_auth_len != current_auth_len {
        msg!(
            "Authority size change not supported during ownership transfer (old={} bytes, new={} bytes). \
             Create a new wallet with the desired authority type instead.",
            current_auth_len,
            new_auth_len,
        );
        return Err(LazorKitError::InvalidInstruction.into());
    }

    // Create new position with updated authority type
    // Note: We can't change size here since that requires data migration
    let mut new_pos = Position::new(new_auth_type, new_auth_len as u16, 0);
    new_pos.boundary = current_boundary;

    // Unsafe cast to mutable reference to write
    let pos_ref = unsafe { Position::load_mut_unchecked(&mut role_buffer[..Position::LEN])? };
    *pos_ref = new_pos; // Direct assignment

    // Write new authority data
    let auth_offset = Position::LEN;
    role_buffer[auth_offset..auth_offset + new_auth_len].copy_from_slice(&new_owner_authority_data);

    msg!(
        "Ownership transferred to new authority type {:?}",
        new_auth_type
    );

    Ok(())
}
