use assertions::{check_zero_data, sol_assert_bytes_eq};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::{
    auth::{ed25519, secp256r1},
    error::AuthError,
    state::{authority::AuthorityAccountHeader, AccountDiscriminator},
};

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() < 1 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let auth_type = instruction_data[0];
    let rest = &instruction_data[1..];

    let (id_seed, full_auth_data) = match auth_type {
        0 => {
            if rest.len() < 32 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (pubkey, _) = rest.split_at(32);
            (pubkey, pubkey)
        },
        1 => {
            if rest.len() < 65 {
                // 32 (hash) + 33 (pubkey) minimum
                return Err(ProgramError::InvalidInstructionData);
            }
            let (hash, _rest_after_hash) = rest.split_at(32);
            let full_data = &rest[..65]; // hash + pubkey (33 bytes)
            (hash, full_data)
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };

    // Split data_payload and authority_payload
    let data_payload_len = 1 + full_auth_data.len(); // auth_type + full_auth_data
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
    let current_owner = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let new_owner = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if wallet_pda.owner() != program_id || current_owner.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    {
        let mut data = unsafe { current_owner.borrow_mut_data_unchecked() };
        let auth = unsafe { &*(data.as_ptr() as *const AuthorityAccountHeader) };
        if auth.discriminator != AccountDiscriminator::Authority as u8 {
            return Err(ProgramError::InvalidAccountData);
        }
        if auth.wallet != *wallet_pda.key() {
            return Err(ProgramError::InvalidAccountData);
        }
        if auth.role != 0 {
            return Err(AuthError::PermissionDenied.into());
        }

        // Authenticate Current Owner
        match auth.authority_type {
            0 => {
                ed25519::authenticate(&data, accounts)?;
            },
            1 => {
                secp256r1::authenticate(
                    &mut data,
                    accounts,
                    authority_payload,
                    data_payload,
                    current_slot,
                )?;
            },
            _ => return Err(AuthError::InvalidAuthenticationKind.into()),
        }
    }

    let (new_key, bump) = find_program_address(
        &[b"authority", wallet_pda.key().as_ref(), id_seed],
        program_id,
    );
    if !sol_assert_bytes_eq(new_owner.key().as_ref(), new_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(new_owner, ProgramError::AccountAlreadyInitialized)?;

    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    let variable_size = if auth_type == 1 {
        4 + full_auth_data.len()
    } else {
        full_auth_data.len()
    };
    let space = header_size + variable_size;
    let rent = 897840 + (space as u64 * 6960);

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
            pubkey: new_owner.key(),
            is_signer: false,
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
        &[&payer.clone(), &new_owner.clone(), &system_program.clone()],
        &[signer],
    )?;

    let data = unsafe { new_owner.borrow_mut_data_unchecked() };
    let header = AuthorityAccountHeader {
        discriminator: AccountDiscriminator::Authority as u8,
        authority_type: auth_type,
        role: 0,
        bump,
        wallet: *wallet_pda.key(),
        _padding: [0; 4],
    };
    unsafe {
        *(data.as_mut_ptr() as *mut AuthorityAccountHeader) = header;
    }

    let variable_target = &mut data[header_size..];
    if auth_type == 1 {
        variable_target[0..4].copy_from_slice(&0u32.to_le_bytes());
        variable_target[4..].copy_from_slice(full_auth_data);
    } else {
        variable_target.copy_from_slice(full_auth_data);
    }

    let current_lamports = unsafe { *current_owner.borrow_mut_lamports_unchecked() };
    let payer_lamports = unsafe { *payer.borrow_mut_lamports_unchecked() };
    unsafe {
        *payer.borrow_mut_lamports_unchecked() = payer_lamports + current_lamports;
        *current_owner.borrow_mut_lamports_unchecked() = 0;
    }
    let current_data = unsafe { current_owner.borrow_mut_data_unchecked() };
    for i in 0..current_data.len() {
        current_data[i] = 0;
    }

    Ok(())
}
