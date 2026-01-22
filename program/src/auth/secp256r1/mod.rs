use crate::{error::AuthError, state::authority::AuthorityAccountHeader};
use core::mem::MaybeUninit;
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::instructions::{Instructions, INSTRUCTIONS_ID},
};
use pinocchio_pubkey::pubkey;

pub mod introspection;
pub mod webauthn;

use introspection::verify_secp256r1_instruction_data; // Removed SECP256R1_PROGRAM_ID
use webauthn::{webauthn_message, R1AuthenticationKind};

/// Maximum age (in slots) for a Secp256r1 signature to be considered valid
const MAX_SIGNATURE_AGE_IN_SLOTS: u64 = 60;
const WEBAUTHN_AUTHENTICATOR_DATA_MAX_SIZE: usize = 196;

/// Authenticates a Secp256r1 authority.
pub fn authenticate(
    auth_data: &mut [u8],
    account_infos: &[AccountInfo],
    authority_payload: &[u8],
    data_payload: &[u8],
    current_slot: u64,
) -> Result<(), ProgramError> {
    if authority_payload.len() < 17 {
        return Err(AuthError::InvalidAuthorityPayload.into());
    }

    let authority_slot = u64::from_le_bytes(unsafe {
        authority_payload
            .get_unchecked(..8)
            .try_into()
            .map_err(|_| AuthError::InvalidAuthorityPayload)?
    });

    let counter = u32::from_le_bytes(unsafe {
        authority_payload
            .get_unchecked(8..12)
            .try_into()
            .map_err(|_| AuthError::InvalidAuthorityPayload)?
    });

    let instruction_account_index = authority_payload[12] as usize;

    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    if auth_data.len() < header_size + 4 {
        return Err(ProgramError::InvalidAccountData);
    }

    let odometer_bytes: [u8; 4] = auth_data[header_size..header_size + 4].try_into().unwrap();
    let odometer = u32::from_le_bytes(odometer_bytes);

    let expected_counter = odometer.wrapping_add(1);
    if counter != expected_counter {
        return Err(AuthError::SignatureReused.into());
    }

    let pubkey_offset = header_size + 4 + 32;
    if auth_data.len() < pubkey_offset + 33 {
        return Err(ProgramError::InvalidAccountData);
    }
    let pubkey_slice = &auth_data[pubkey_offset..pubkey_offset + 33];
    let pubkey: [u8; 33] = pubkey_slice
        .try_into()
        .map_err(|_| ProgramError::InvalidAccountData)?;

    secp256r1_authenticate(
        &pubkey,
        data_payload,
        authority_slot,
        current_slot,
        account_infos,
        instruction_account_index,
        counter,
        &authority_payload[17..],
    )?;

    let new_odometer_bytes = counter.to_le_bytes();
    auth_data[header_size..header_size + 4].copy_from_slice(&new_odometer_bytes);

    Ok(())
}

fn secp256r1_authenticate(
    expected_key: &[u8; 33],
    data_payload: &[u8],
    authority_slot: u64,
    current_slot: u64,
    account_infos: &[AccountInfo],
    instruction_account_index: usize,
    counter: u32,
    additional_payload: &[u8],
) -> Result<(), ProgramError> {
    if current_slot < authority_slot || current_slot - authority_slot > MAX_SIGNATURE_AGE_IN_SLOTS {
        return Err(AuthError::InvalidSignatureAge.into());
    }

    let computed_hash = compute_message_hash(data_payload, account_infos, authority_slot, counter)?;

    let mut message_buf: MaybeUninit<[u8; WEBAUTHN_AUTHENTICATOR_DATA_MAX_SIZE + 32]> =
        MaybeUninit::uninit();

    let message = if additional_payload.is_empty() {
        &computed_hash
    } else {
        let r1_auth_kind = u16::from_le_bytes(additional_payload[..2].try_into().unwrap());

        match r1_auth_kind.try_into()? {
            R1AuthenticationKind::WebAuthn => {
                webauthn_message(additional_payload, computed_hash, unsafe {
                    &mut *message_buf.as_mut_ptr()
                })?
            },
        }
    };

    let sysvar_instructions = account_infos
        .get(instruction_account_index)
        .ok_or(AuthError::InvalidAuthorityPayload)?;

    if sysvar_instructions.key().as_ref() != &INSTRUCTIONS_ID {
        return Err(AuthError::InvalidInstruction.into());
    }

    let sysvar_instructions_data = unsafe { sysvar_instructions.borrow_data_unchecked() };
    let ixs = unsafe { Instructions::new_unchecked(sysvar_instructions_data) };
    let current_index = ixs.load_current_index() as usize;
    if current_index == 0 {
        return Err(AuthError::InvalidInstruction.into());
    }

    let secpr1ix = unsafe { ixs.deserialize_instruction_unchecked(current_index - 1) };

    let program_id = secpr1ix.get_program_id();
    if program_id != &pubkey!("Secp256r1SigVerify1111111111111111111111111") {
        return Err(AuthError::InvalidInstruction.into());
    }

    let instruction_data = secpr1ix.get_instruction_data();
    verify_secp256r1_instruction_data(&instruction_data, expected_key, message)?;
    Ok(())
}

fn compute_message_hash(
    data_payload: &[u8],
    _account_infos: &[AccountInfo],
    authority_slot: u64,
    counter: u32,
) -> Result<[u8; 32], ProgramError> {
    let mut hash = MaybeUninit::<[u8; 32]>::uninit();

    let slot_bytes = authority_slot.to_le_bytes();
    let counter_bytes = counter.to_le_bytes();

    let _data = [data_payload, &slot_bytes, &counter_bytes];

    #[cfg(target_os = "solana")]
    unsafe {
        let _res = pinocchio::syscalls::sol_sha256(
            _data.as_ptr() as *const u8,
            _data.len() as u64,
            hash.as_mut_ptr() as *mut u8,
        );
    }
    #[cfg(not(target_os = "solana"))]
    {
        // Mock hash for local test
        unsafe {
            *hash.as_mut_ptr() = [0u8; 32];
        }
    }

    Ok(unsafe { hash.assume_init() })
}
