//! TransferOwnership instruction handler

use lazorkit_state::{
    authority::authority_type_to_length, read_position, AuthorityType, LazorKitWallet, Position,
    Transmutable, TransmutableMut,
};
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

use crate::error::LazorKitError;

pub fn process_transfer_ownership(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_owner_authority_type: u16,
    new_owner_authority_data: Vec<u8>,
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

    // Read current owner position using Zero-Copy helper (assuming read_position is efficient/zero-copy safe)
    // read_position should be updated to use load_unchecked if it's not already.
    // Assuming read_position takes slice and returns Position.
    let current_pos = read_position(role_buffer)?;
    if current_pos.id != 0 {
        msg!("First role is not Owner");
        return Err(LazorKitError::InvalidWalletAccount.into());
    }

    // Check if size changes
    let current_auth_len = current_pos.authority_length as usize;

    // Verify signer matches current owner authority
    let current_auth_type = AuthorityType::try_from(current_pos.authority_type)?;
    let current_auth_data = &role_buffer[Position::LEN..Position::LEN + current_auth_len];

    match current_auth_type {
        AuthorityType::Ed25519 | AuthorityType::Ed25519Session => {
            // First 32 bytes are the public key
            let expected_pubkey = &current_auth_data[..32];
            if owner_account.key().as_ref() != expected_pubkey {
                msg!("Signer does not match current owner");
                return Err(ProgramError::MissingRequiredSignature);
            }
        },
        _ => {
            // TransferOwnership currently only supports Ed25519 authorities
            // Other types (Secp256k1/r1/ProgramExec) require signature payload
            // which is not included in the current instruction format
            msg!(
                "TransferOwnership only supports Ed25519 authorities (current type: {:?})",
                current_auth_type
            );
            return Err(LazorKitError::InvalidInstruction.into());
        },
    }

    if new_auth_len != current_auth_len {
        msg!(
            "Authority size change not supported yet (old={}, new={})",
            current_auth_len,
            new_auth_len
        );
        return Err(LazorKitError::InvalidInstruction.into());
    }

    // Update Position header (Zero-Copy)
    let new_pos = Position {
        authority_type: new_owner_authority_type,
        authority_length: new_auth_len as u16,
        num_policies: current_pos.num_policies,
        padding: 0,
        id: 0,
        boundary: current_pos.boundary,
    };

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
