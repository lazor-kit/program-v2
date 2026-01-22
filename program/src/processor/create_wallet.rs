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
    error::AuthError,
    state::{authority::AuthorityAccountHeader, wallet::WalletAccount, AccountDiscriminator},
};

#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct CreateWalletArgs {
    pub user_seed: [u8; 32],
    pub authority_type: u8,
    pub auth_bump: u8,
    pub _padding: [u8; 6], // 32+1+1+6 = 40 bytes
}

impl CreateWalletArgs {
    pub fn from_bytes(data: &[u8]) -> Result<(&Self, &[u8]), ProgramError> {
        if data.len() < 40 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let (fixed, rest) = data.split_at(40);
        let args = unsafe { &*(fixed.as_ptr() as *const CreateWalletArgs) };
        Ok((args, rest))
    }
}

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (args, rest) = CreateWalletArgs::from_bytes(instruction_data)?;

    let (id_seed, full_auth_data) = match args.authority_type {
        0 => {
            if rest.len() != 32 {
                return Err(ProgramError::InvalidInstructionData);
            }
            (rest, rest)
        },
        1 => {
            if rest.len() < 32 {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (hash, _key) = rest.split_at(32);
            (hash, rest)
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    };

    let account_info_iter = &mut accounts.iter();
    let payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let vault_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    let (wallet_key, wallet_bump) = find_program_address(&[b"wallet", &args.user_seed], program_id);
    if !sol_assert_bytes_eq(wallet_pda.key().as_ref(), wallet_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(wallet_pda, ProgramError::AccountAlreadyInitialized)?;

    let (vault_key, _vault_bump) =
        find_program_address(&[b"vault", wallet_key.as_ref()], program_id);
    if !sol_assert_bytes_eq(vault_pda.key().as_ref(), vault_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    let (auth_key, auth_bump) =
        find_program_address(&[b"authority", wallet_key.as_ref(), id_seed], program_id);
    if !sol_assert_bytes_eq(auth_pda.key().as_ref(), auth_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(auth_pda, ProgramError::AccountAlreadyInitialized)?;

    // --- Init Wallet Account ---
    let wallet_space = 8;
    let wallet_rent = 897840 + (wallet_space as u64 * 6960);

    let mut create_wallet_ix_data = Vec::with_capacity(52);
    create_wallet_ix_data.extend_from_slice(&0u32.to_le_bytes());
    create_wallet_ix_data.extend_from_slice(&wallet_rent.to_le_bytes());
    create_wallet_ix_data.extend_from_slice(&(wallet_space as u64).to_le_bytes());
    create_wallet_ix_data.extend_from_slice(program_id.as_ref());

    let wallet_accounts_meta = [
        AccountMeta {
            pubkey: payer.key(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: wallet_pda.key(),
            is_signer: false,
            is_writable: true,
        },
    ];
    let create_wallet_ix = Instruction {
        program_id: system_program.key(),
        accounts: &wallet_accounts_meta,
        data: &create_wallet_ix_data,
    };
    let wallet_bump_arr = [wallet_bump];
    let wallet_seeds = [
        Seed::from(b"wallet"),
        Seed::from(&args.user_seed),
        Seed::from(&wallet_bump_arr),
    ];
    let wallet_signer: Signer = (&wallet_seeds).into();

    invoke_signed(
        &create_wallet_ix,
        &[&payer.clone(), &wallet_pda.clone(), &system_program.clone()],
        &[wallet_signer],
    )?;

    // Write Wallet Data
    let wallet_data = unsafe { wallet_pda.borrow_mut_data_unchecked() };
    let wallet_account = WalletAccount {
        discriminator: AccountDiscriminator::Wallet as u8,
        bump: wallet_bump,
        _padding: [0; 6],
    };
    unsafe {
        *(wallet_data.as_mut_ptr() as *mut WalletAccount) = wallet_account;
    }

    // --- Init Authority Account ---
    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    let variable_size = if args.authority_type == 1 {
        4 + full_auth_data.len()
    } else {
        full_auth_data.len()
    };

    let auth_space = header_size + variable_size;
    let auth_rent = 897840 + (auth_space as u64 * 6960);

    let mut create_auth_ix_data = Vec::with_capacity(52);
    create_auth_ix_data.extend_from_slice(&0u32.to_le_bytes());
    create_auth_ix_data.extend_from_slice(&auth_rent.to_le_bytes());
    create_auth_ix_data.extend_from_slice(&(auth_space as u64).to_le_bytes());
    create_auth_ix_data.extend_from_slice(program_id.as_ref());

    let auth_accounts_meta = [
        AccountMeta {
            pubkey: payer.key(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: auth_pda.key(),
            is_signer: false,
            is_writable: true,
        },
    ];
    let create_auth_ix = Instruction {
        program_id: system_program.key(),
        accounts: &auth_accounts_meta,
        data: &create_auth_ix_data,
    };
    let auth_bump_arr = [auth_bump];
    let auth_seeds = [
        Seed::from(b"authority"),
        Seed::from(wallet_key.as_ref()),
        Seed::from(id_seed),
        Seed::from(&auth_bump_arr),
    ];
    let auth_signer: Signer = (&auth_seeds).into();

    invoke_signed(
        &create_auth_ix,
        &[&payer.clone(), &auth_pda.clone(), &system_program.clone()],
        &[auth_signer],
    )?;

    // Write Authority Data
    let auth_account_data = unsafe { auth_pda.borrow_mut_data_unchecked() };

    let header = AuthorityAccountHeader {
        discriminator: AccountDiscriminator::Authority as u8,
        authority_type: args.authority_type,
        role: 0,
        bump: auth_bump,
        wallet: *wallet_pda.key(),
        _padding: [0; 4],
    };
    unsafe {
        *(auth_account_data.as_mut_ptr() as *mut AuthorityAccountHeader) = header;
    }

    let variable_target = &mut auth_account_data[header_size..];

    if args.authority_type == 1 {
        variable_target[0..4].copy_from_slice(&0u32.to_le_bytes());
        variable_target[4..].copy_from_slice(full_auth_data);
    } else {
        variable_target.copy_from_slice(full_auth_data);
    }

    Ok(())
}
