use assertions::{check_zero_data, sol_assert_bytes_eq};
use no_padding::NoPadding;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    ProgramResult,
};

use crate::{
    auth::{
        ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator,
    },
    error::AuthError,
    state::{authority::AuthorityAccountHeader, AccountDiscriminator},
};

#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct AddAuthorityArgs {
    pub authority_type: u8,
    pub new_role: u8,
    pub _padding: [u8; 6],
}

impl AddAuthorityArgs {
    pub fn from_bytes(data: &[u8]) -> Result<(&Self, &[u8]), ProgramError> {
        if data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let (fixed, rest) = data.split_at(8);
        let args = unsafe { &*(fixed.as_ptr() as *const AddAuthorityArgs) };
        Ok((args, rest))
    }
}

pub fn process_add_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (args, rest) = AddAuthorityArgs::from_bytes(instruction_data)?;

    let (id_seed, full_auth_data) = match args.authority_type {
        0 => {
            if rest.len() < 32 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (pubkey, _) = rest.split_at(32);
            (pubkey, pubkey)
        },
        1 => {
            if rest.len() < 32 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (hash, rest_after_hash) = rest.split_at(32);
            // For Secp256r1: need hash + pubkey for full_auth_data
            // Pubkey is variable but typically 33 bytes (compressed)
            // We need to determine where auth_data ends and authority_payload begins
            // Assuming fixed 33 bytes for pubkey
            if rest_after_hash.len() < 33 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let full_data = &rest[..32 + 33]; // hash + pubkey
            (hash, full_data)
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };

    // Split data_payload and authority_payload
    // data_payload = everything up to and including the new authority data
    let data_payload_len = 8 + full_auth_data.len(); // args + full_auth_data
    if instruction_data.len() < data_payload_len {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (data_payload, authority_payload) = instruction_data.split_at(data_payload_len);

    let account_info_iter = &mut accounts.iter();
    let payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let admin_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let new_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if wallet_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    if admin_auth_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    let mut admin_data = unsafe { admin_auth_pda.borrow_mut_data_unchecked() };
    if admin_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
        return Err(ProgramError::InvalidAccountData);
    }

    let admin_header = unsafe { &*(admin_data.as_ptr() as *const AuthorityAccountHeader) };

    if admin_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if admin_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Unified Authentication
    match admin_header.authority_type {
        0 => {
            // Ed25519: Verify signer (authority_payload ignored)
            Ed25519Authenticator.authenticate(accounts, &mut admin_data, &[], &[])?;
        },
        1 => {
            // Secp256r1: Full authentication with payload
            Secp256r1Authenticator.authenticate(
                accounts,
                &mut admin_data,
                authority_payload,
                data_payload,
            )?;
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    }

    // Authorization
    if admin_header.role != 0 && (admin_header.role != 1 || args.new_role != 2) {
        return Err(AuthError::PermissionDenied.into());
    }

    // Logic
    let (new_auth_key, bump) = find_program_address(
        &[b"authority", wallet_pda.key().as_ref(), id_seed],
        program_id,
    );
    if !sol_assert_bytes_eq(new_auth_pda.key().as_ref(), new_auth_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(new_auth_pda, ProgramError::AccountAlreadyInitialized)?;

    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    let space = header_size + full_auth_data.len();
    let rent = (space as u64)
        .checked_mul(6960)
        .and_then(|val| val.checked_add(897840))
        .ok_or(ProgramError::ArithmeticOverflow)?;

    // ... (create_ix logic same) ...

    let mut create_ix_data = Vec::with_capacity(52);
    create_ix_data.extend_from_slice(&0u32.to_le_bytes());
    create_ix_data.extend_from_slice(&rent.to_le_bytes());
    create_ix_data.extend_from_slice(&(space as u64).to_le_bytes());
    create_ix_data.extend_from_slice(program_id.as_ref());

    let accounts_meta = [
        AccountMeta {
            pubkey: payer.key(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: new_auth_pda.key(),
            is_signer: true, // Must be true even with invoke_signed
            is_writable: true,
        },
    ];
    let create_ix = Instruction {
        program_id: system_program.key(),
        accounts: &accounts_meta,
        data: &create_ix_data,
    };
    let bump_arr = [bump];
    let seeds = [
        Seed::from(b"authority"),
        Seed::from(wallet_pda.key().as_ref()),
        Seed::from(id_seed),
        Seed::from(&bump_arr),
    ];
    let signer: Signer = (&seeds).into();

    invoke_signed(
        &create_ix,
        &[
            &payer.clone(),
            &new_auth_pda.clone(),
            &system_program.clone(),
        ],
        &[signer],
    )?;

    let data = unsafe { new_auth_pda.borrow_mut_data_unchecked() };
    let header = AuthorityAccountHeader {
        discriminator: AccountDiscriminator::Authority as u8,
        authority_type: args.authority_type,
        role: args.new_role,
        bump,
        _padding: [0; 4],
        counter: 0,
        wallet: *wallet_pda.key(),
    };
    unsafe {
        *(data.as_mut_ptr() as *mut AuthorityAccountHeader) = header;
    }

    let variable_target = &mut data[header_size..];
    variable_target.copy_from_slice(full_auth_data);

    Ok(())
}

pub fn process_remove_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // For RemoveAuthority, all instruction_data is authority_payload
    // data_payload is empty (or could be just the discriminator)
    let authority_payload = instruction_data;
    let data_payload = &[]; // Empty for remove

    let account_info_iter = &mut accounts.iter();
    let _payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let admin_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let target_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let refund_dest = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if wallet_pda.owner() != program_id
        || admin_auth_pda.owner() != program_id
        || target_auth_pda.owner() != program_id
    {
        return Err(ProgramError::IllegalOwner);
    }

    let mut admin_data = unsafe { admin_auth_pda.borrow_mut_data_unchecked() };
    let admin_header = unsafe { &*(admin_data.as_ptr() as *const AuthorityAccountHeader) };
    if admin_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if admin_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Authentication
    match admin_header.authority_type {
        0 => {
            Ed25519Authenticator.authenticate(accounts, &mut admin_data, &[], &[])?;
        },
        1 => {
            Secp256r1Authenticator.authenticate(
                accounts,
                &mut admin_data,
                authority_payload,
                data_payload,
            )?;
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    }

    // Authorization
    if admin_header.role != 0 {
        let target_data = unsafe { target_auth_pda.borrow_data_unchecked() };
        let target_header = unsafe { &*(target_data.as_ptr() as *const AuthorityAccountHeader) };
        if target_header.discriminator != AccountDiscriminator::Authority as u8 {
            return Err(ProgramError::InvalidAccountData);
        }

        if admin_header.role != 1 || target_header.role != 2 {
            return Err(AuthError::PermissionDenied.into());
        }
    }

    let target_lamports = unsafe { *target_auth_pda.borrow_mut_lamports_unchecked() };
    let refund_lamports = unsafe { *refund_dest.borrow_mut_lamports_unchecked() };
    unsafe {
        *refund_dest.borrow_mut_lamports_unchecked() = refund_lamports
            .checked_add(target_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *target_auth_pda.borrow_mut_lamports_unchecked() = 0;
    }
    let target_data = unsafe { target_auth_pda.borrow_mut_data_unchecked() };
    for i in 0..target_data.len() {
        target_data[i] = 0;
    }

    Ok(())
}
