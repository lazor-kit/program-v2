//! Execute instruction handler - Simplified

use alloc::vec::Vec; // Kept for traits/macros if needed, but avoiding usage
use core::mem::MaybeUninit;
use lazorkit_state::authority::{
    ed25519::{Ed25519Authority, Ed25519SessionAuthority},
    secp256r1::{Secp256r1Authority, Secp256r1SessionAuthority},
    AuthorityInfo, AuthorityType,
};
use lazorkit_state::{
    read_position, IntoBytes, LazorKitWallet, Position, Transmutable, TransmutableMut,
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Account, AccountMeta, Instruction, Seed, Signer},
    msg,
    program::invoke_signed_unchecked,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::error::LazorKitError;

#[cfg(target_os = "solana")]
extern "C" {
    fn sol_get_return_data(data: *mut u8, length: u64, program_id: *mut Pubkey) -> u64;
}

#[cfg(not(target_os = "solana"))]
unsafe fn sol_get_return_data(_data: *mut u8, _length: u64, _program_id: *mut Pubkey) -> u64 {
    0
}

/// Helper to dispatch call to invoke_signed with a variable number of accounts.
fn dispatch_invoke_signed(
    instruction: &Instruction,
    accounts: &[&AccountInfo],
    signers_seeds: &[Signer],
) -> ProgramResult {
    const MAX_ACCOUNTS: usize = 24;
    let count = accounts.len();
    if count > MAX_ACCOUNTS {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let mut accounts_storage: [MaybeUninit<Account>; MAX_ACCOUNTS] =
        unsafe { MaybeUninit::uninit().assume_init() };

    for (i, info) in accounts.iter().enumerate() {
        accounts_storage[i].write(Account::from(*info));
    }

    let account_structs =
        unsafe { core::slice::from_raw_parts(accounts_storage.as_ptr() as *const Account, count) };

    unsafe { invoke_signed_unchecked(instruction, account_structs, signers_seeds) };
    Ok(())
}

/// Helper to scan for a specific role in the wallet registry.
fn find_role(config_data: &[u8], role_id: u32) -> Result<(Position, usize), ProgramError> {
    let mut current_cursor = LazorKitWallet::LEN;
    let wallet = unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN]) }
        .map_err(|_| ProgramError::InvalidAccountData)?;
    let mut remaining = wallet.role_count;

    while remaining > 0 {
        if current_cursor + Position::LEN > config_data.len() {
            break;
        }
        let pos_ref = read_position(&config_data[current_cursor..])?;
        if pos_ref.id == role_id {
            return Ok((*pos_ref, current_cursor));
        }
        current_cursor = pos_ref.boundary as usize;
        remaining -= 1;
    }
    Err(LazorKitError::AuthorityNotFound.into())
}

pub fn process_execute(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    role_id: u32,
    instruction_payload_len: u16,
    unified_payload: &[u8],
) -> ProgramResult {
    let mut account_info_iter = accounts.iter();
    let config_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let vault_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let _system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if config_account.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // --- Phase 1: Immutable Scan ---
    let (pos, role_abs_offset, wallet_bump) = {
        let config_data = config_account.try_borrow_data()?;
        let wallet = unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN]) }
            .map_err(|_| ProgramError::InvalidAccountData)?;

        if !wallet.is_valid() {
            return Err(ProgramError::InvalidAccountData);
        }

        let vault_bump = wallet.wallet_bump;

        let seeds = &[
            b"lazorkit-wallet-address",
            config_account.key().as_ref(),
            &[vault_bump],
        ];
        let derived_vault = pinocchio::pubkey::create_program_address(seeds, program_id)
            .map_err(|_| LazorKitError::InvalidPDA)?;
        if derived_vault != *vault_account.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        if vault_account.owner() != &pinocchio_system::ID {
            return Err(ProgramError::IllegalOwner);
        }

        let (pos, offset) = find_role(&config_data, role_id)?;
        (pos, offset, vault_bump)
    };
    let slot = Clock::get()?.slot;

    if unified_payload.len() < instruction_payload_len as usize {
        return Err(ProgramError::InvalidInstructionData);
    }
    // --- Phase 2: Mutable Process ---
    let mut config_data = config_account.try_borrow_mut_data()?;

    let (execution_data, auth_payload) = unified_payload.split_at(instruction_payload_len as usize);

    let auth_start = role_abs_offset + Position::LEN;
    let auth_end = auth_start + pos.authority_length as usize;
    if auth_end > config_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }

    // === 1. AUTHENTICATION ===
    let mut exclude_signer_index: Option<usize> = None;
    {
        let authority_data_slice = &mut config_data[auth_start..auth_end];
        let auth_type = AuthorityType::try_from(pos.authority_type)?;

        if matches!(
            auth_type,
            AuthorityType::Ed25519 | AuthorityType::Ed25519Session
        ) {
            if let Some(&idx) = auth_payload.first() {
                exclude_signer_index = Some(idx as usize);
            }
        }

        macro_rules! authenticate_auth {
            ($auth_type:ty) => {{
                let mut auth = unsafe { <$auth_type>::load_mut_unchecked(authority_data_slice) }
                    .map_err(|_| ProgramError::InvalidAccountData)?;
                if auth.session_based() {
                    auth.authenticate_session(accounts, auth_payload, execution_data, slot)?;
                } else {
                    auth.authenticate(accounts, auth_payload, execution_data, slot)?;
                }
            }};
        }

        match auth_type {
            AuthorityType::Ed25519 => authenticate_auth!(Ed25519Authority),
            AuthorityType::Ed25519Session => authenticate_auth!(Ed25519SessionAuthority),
            AuthorityType::Secp256r1 => authenticate_auth!(Secp256r1Authority),
            AuthorityType::Secp256r1Session => authenticate_auth!(Secp256r1SessionAuthority),
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    }

    // === 2. PREPARE TARGET EXECUTION ===
    if accounts.len() < 4 {
        msg!("Missing target program account");
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let target_program = &accounts[3];

    // SECURITY: Verify target program is executable
    if !target_program.executable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Stack based account meta collection
    const MAX_METAS: usize = 24;
    let mut metas_storage: [MaybeUninit<AccountMeta>; MAX_METAS] =
        unsafe { MaybeUninit::uninit().assume_init() };

    // We also need parallel array of references for dispatch
    // invoke_accounts[0] is program
    let mut invoke_accounts_storage: [MaybeUninit<&AccountInfo>; MAX_METAS + 1] =
        unsafe { MaybeUninit::uninit().assume_init() };

    invoke_accounts_storage[0].write(target_program);
    let mut meta_count = 0;

    for (i, acc) in accounts[4..].iter().enumerate() {
        if meta_count >= MAX_METAS {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let abs_index = 4 + i;

        let should_be_signer = if Some(abs_index) == exclude_signer_index {
            false
        } else {
            acc.is_signer()
        };

        let mut meta = unsafe {
            // Manual construction or use AccountMeta methods.
            // pinocchio AccountMeta fields are pub.
            AccountMeta {
                pubkey: acc.key(),
                is_signer: should_be_signer,
                is_writable: acc.is_writable(),
            }
        };

        if acc.key() == vault_account.key() {
            meta.is_signer = true;
        }

        metas_storage[meta_count].write(meta);
        invoke_accounts_storage[meta_count + 1].write(acc);
        meta_count += 1;
    }

    let target_account_metas = unsafe {
        core::slice::from_raw_parts(metas_storage.as_ptr() as *const AccountMeta, meta_count)
    };

    let invoke_accounts = unsafe {
        core::slice::from_raw_parts(
            invoke_accounts_storage.as_ptr() as *const &AccountInfo,
            meta_count + 1,
        )
    };

    // === 3. EXECUTE PAYLOAD ===
    let execute_instruction = Instruction {
        program_id: target_program.key(),
        accounts: target_account_metas,
        data: execution_data,
    };

    let seeds = &[
        b"lazorkit-wallet-address",
        config_account.key().as_ref(),
        &[wallet_bump],
    ];
    let seed_list = [
        Seed::from(seeds[0]),
        Seed::from(seeds[1]),
        Seed::from(seeds[2]),
    ];
    let signer = Signer::from(&seed_list);
    let signers_seeds = &[signer];

    // === 6. DISPATCH ===
    dispatch_invoke_signed(&execute_instruction, invoke_accounts, signers_seeds)?;
    Ok(())
}
