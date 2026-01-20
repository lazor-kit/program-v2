//! Execute instruction handler - Simplified

use alloc::vec::Vec;
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
    let mut account_structs = Vec::with_capacity(accounts.len());
    for info in accounts {
        account_structs.push(Account::from(*info));
    }

    unsafe { invoke_signed_unchecked(instruction, &account_structs, signers_seeds) };
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
        let vault_bump = wallet.wallet_bump;

        // Verify seeds
        let seeds = &[
            b"lazorkit-wallet-address",
            config_account.key().as_ref(),
            &[vault_bump],
        ];
        let derived_vault = pinocchio::pubkey::create_program_address(seeds, program_id)
            .map_err(|_| LazorKitError::InvalidPDA)?;
        if derived_vault != *vault_account.key() {
            msg!("Execute: Mismatched vault seeds");
            msg!("Config Key: {:?}", config_account.key());
            msg!("Provided Vault Key: {:?}", vault_account.key());
            msg!("Derived Vault Key: {:?}", derived_vault);
            msg!("Wallet Bump: {}", vault_bump);
            return Err(ProgramError::InvalidAccountData);
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

        // Macro to simplify auth calls
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
        msg!("Target program is not executable");
        return Err(ProgramError::InvalidAccountData);
    }

    let target_instruction_data = execution_data.to_vec();

    let mut target_account_metas = Vec::new();
    let mut invoke_accounts = Vec::with_capacity(1 + accounts.len().saturating_sub(4));

    // IMPORTANT: CPI requires the executable account to be in the invoke list
    invoke_accounts.push(target_program);

    for (i, acc) in accounts[4..].iter().enumerate() {
        let abs_index = 4 + i;
        if Some(abs_index) == exclude_signer_index {
            continue;
        }
        let mut meta = AccountMeta {
            pubkey: acc.key(),
            is_signer: acc.is_signer(),
            is_writable: acc.is_writable(),
        };
        if acc.key() == vault_account.key() {
            meta.is_signer = true;
        }
        target_account_metas.push(meta);
        invoke_accounts.push(acc);
    }

    // === 3. EXECUTE PAYLOAD ===
    let execute_instruction = Instruction {
        program_id: target_program.key(),
        accounts: &target_account_metas,
        data: &target_instruction_data,
    };

    let seeds = &[
        b"lazorkit-wallet-address",
        config_account.key().as_ref(), // expected_config
        &[wallet_bump],
    ];
    let seed_list = [
        Seed::from(seeds[0]),
        Seed::from(seeds[1]),
        Seed::from(seeds[2]),
    ];
    let signer = Signer::from(&seed_list);
    let signers_seeds = &[signer]; // Array of Signer

    // === 6. DISPATCH ===
    msg!(
        "Dispatching target execution. Data len: {}",
        target_instruction_data.len()
    );

    dispatch_invoke_signed(&execute_instruction, &invoke_accounts, signers_seeds)?;
    Ok(())
}
