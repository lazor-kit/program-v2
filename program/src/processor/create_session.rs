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
    state::{authority::AuthorityAccountHeader, session::SessionAccount, AccountDiscriminator},
};

#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct CreateSessionArgs {
    pub session_key: [u8; 32],
    pub expires_at: u64,
}

impl CreateSessionArgs {
    pub fn from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        if data.len() < 40 {
            return Err(ProgramError::InvalidInstructionData);
        }
        // args are: [session_key(32)][expires_at(8)]
        let args = unsafe { &*(data.as_ptr() as *const CreateSessionArgs) };
        Ok(args)
    }
}

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let args = CreateSessionArgs::from_bytes(instruction_data)?;

    let account_info_iter = &mut accounts.iter();
    let payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let authorizer_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let session_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if wallet_pda.owner() != program_id || authorizer_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Verify Authorizer
    let mut auth_data = unsafe { authorizer_pda.borrow_mut_data_unchecked() };
    let auth_header = unsafe { &*(auth_data.as_ptr() as *const AuthorityAccountHeader) };

    if auth_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if auth_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }
    // Only Admin (1) or Owner (0) can create sessions.
    // Spender (2) cannot create sessions.
    if auth_header.role != 0 && auth_header.role != 1 {
        return Err(AuthError::PermissionDenied.into());
    }

    // Authenticate Authorizer

    // We assume CreateSession instruction data AFTER the args is payload for Secp256r1 if any
    let payload_offset = std::mem::size_of::<CreateSessionArgs>();
    let authority_payload = if instruction_data.len() > payload_offset {
        &instruction_data[payload_offset..]
    } else {
        &[]
    };

    // But wait, `CreateSessionArgs` consumes 40 bytes.
    // `instruction_data` passed here is whatever follows the discriminator.
    // `Execute` passes compact instructions.
    // Here we pass args.

    // For Secp256r1, we need to distinguish args from auth payload.
    // The instruction format is [discriminator][args][payload].
    // `instruction_data` here is [args][payload].

    match auth_header.authority_type {
        0 => {
            Ed25519Authenticator.authenticate(accounts, &mut auth_data, &[], &[])?;
        },
        1 => {
            Secp256r1Authenticator.authenticate(
                accounts,
                &mut auth_data,
                authority_payload,
                &instruction_data[..payload_offset], // Sign over args part?
            )?;
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    }

    // Derive Session PDA
    let (session_key, bump) = find_program_address(
        &[b"session", wallet_pda.key().as_ref(), &args.session_key],
        program_id,
    );
    if !sol_assert_bytes_eq(session_pda.key().as_ref(), session_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(session_pda, ProgramError::AccountAlreadyInitialized)?;

    // Create Session Account
    let space = std::mem::size_of::<SessionAccount>();
    // Rent: 897840 + (space * 6960)
    let rent = (space as u64)
        .checked_mul(6960)
        .and_then(|val| val.checked_add(897840))
        .ok_or(ProgramError::ArithmeticOverflow)?;

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
            pubkey: session_pda.key(),
            is_signer: true,
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
        Seed::from(b"session"),
        Seed::from(wallet_pda.key().as_ref()),
        Seed::from(&args.session_key),
        Seed::from(&bump_arr),
    ];
    let signer: Signer = (&seeds).into();

    invoke_signed(
        &create_ix,
        &[
            &payer.clone(),
            &session_pda.clone(),
            &system_program.clone(),
        ],
        &[signer],
    )?;

    // Initialize Session State
    let data = unsafe { session_pda.borrow_mut_data_unchecked() };
    let session = SessionAccount {
        discriminator: AccountDiscriminator::Session as u8,
        bump,
        _padding: [0; 6],
        wallet: *wallet_pda.key(),
        session_key: Pubkey::from(args.session_key),
        expires_at: args.expires_at,
    };
    unsafe {
        *(data.as_mut_ptr() as *mut SessionAccount) = session;
    }

    Ok(())
}
