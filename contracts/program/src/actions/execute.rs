//! Execute instruction handler (Bounce Flow)

use alloc::vec::Vec;
use lazorkit_interface::{VerifyInstruction, INSTRUCTION_VERIFY};
use lazorkit_state::authority::{
    ed25519::{Ed25519Authority, Ed25519SessionAuthority},
    programexec::{ProgramExecAuthority, ProgramExecSessionAuthority},
    secp256k1::{Secp256k1Authority, Secp256k1SessionAuthority},
    secp256r1::{Secp256r1Authority, Secp256r1SessionAuthority},
    AuthorityInfo, AuthorityType,
};
use lazorkit_state::{
    policy::parse_policies, read_position, IntoBytes, LazorKitWallet, Position, Transmutable,
    TransmutableMut,
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
    instruction_payload: &[u8],
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
            .expect("Derived vault seeds failed");
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

    if instruction_payload.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (auth_payload, execution_data) = instruction_payload.split_at(1);

    // --- Phase 2: Mutable Process ---
    let mut config_data = config_account.try_borrow_mut_data()?;

    let auth_start = role_abs_offset + Position::LEN;
    let auth_end = auth_start + pos.authority_length as usize;
    if auth_end > config_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Policies Data Bounds
    let policies_start_offset = auth_end;
    let policies_end_offset = pos.boundary as usize;
    if policies_end_offset > config_data.len() {
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
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            }};
        }

        match auth_type {
            AuthorityType::Ed25519 => authenticate_auth!(Ed25519Authority),
            AuthorityType::Ed25519Session => authenticate_auth!(Ed25519SessionAuthority),
            AuthorityType::Secp256k1 => authenticate_auth!(Secp256k1Authority),
            AuthorityType::Secp256k1Session => authenticate_auth!(Secp256k1SessionAuthority),
            AuthorityType::Secp256r1 => authenticate_auth!(Secp256r1Authority),
            AuthorityType::Secp256r1Session => authenticate_auth!(Secp256r1SessionAuthority),
            AuthorityType::ProgramExec => authenticate_auth!(ProgramExecAuthority),
            AuthorityType::ProgramExecSession => authenticate_auth!(ProgramExecSessionAuthority),
            AuthorityType::None => return Err(ProgramError::InvalidInstructionData),
        }
    }

    // === 2. SCAN POLICIES ===
    let policy_cpi_infos = {
        let policies_slice = &config_data[policies_start_offset..policies_end_offset];
        let mut infos = Vec::new();
        for policy_result in parse_policies(policies_slice) {
            let policy_view = policy_result.map_err(|_| ProgramError::InvalidAccountData)?;
            let pid = policy_view.header.program_id();
            let state_offset = policies_start_offset
                + policy_view.offset
                + lazorkit_state::policy::PolicyHeader::LEN;
            infos.push((pid, state_offset as u32));
        }
        infos
    };

    drop(config_data);

    // === 3. PREPARE TARGET ACCOUNTS ===
    if accounts.len() < 4 {
        msg!("Missing target program account");
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let target_program = &accounts[3];
    let target_instruction_data = execution_data.to_vec();

    let mut target_account_metas = Vec::new();
    // Capacity: 1 (program) + remaining accounts
    let mut invoke_accounts = Vec::with_capacity(1 + accounts.len().saturating_sub(4));

    // IMPORTANT: CPI requires the executable account to be in the invoke list
    invoke_accounts.push(target_program);

    for (i, acc) in accounts[4..].iter().enumerate() {
        let abs_index = 4 + i;
        if Some(abs_index) == exclude_signer_index {
            continue;
        }
        if policy_cpi_infos.iter().any(|(pid, _)| pid == acc.key()) {
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

    // === 4. POLICY ENFORCEMENT ===
    if role_id != 0 && pos.num_policies > 0 {
        let mut policy_found = false;
        msg!("Checking policies. Accounts len: {}", accounts.len());
        for acc in accounts {
            msg!("Acc: {:?}", acc.key());
        }
        for (policy_program_id, state_offset) in &policy_cpi_infos {
            if let Some(policy_acc) = accounts.iter().find(|a| a.key() == policy_program_id) {
                policy_found = true;
                msg!("Calling enforce_single_policy");
                enforce_single_policy(
                    policy_acc,
                    *state_offset,
                    pos.id,
                    slot,
                    execution_data,
                    config_account,
                    vault_account,
                    &target_account_metas,
                    accounts,
                )?;
                msg!("enforce_single_policy success");
            } else {
                msg!("Required policy account not found: {:?}", policy_program_id);
                return Err(ProgramError::NotEnoughAccountKeys);
            }
        }
        if !policy_found {
            msg!("No policy found in accounts loop");
            return Err(ProgramError::InvalidArgument);
        }
    }

    // === 5. EXECUTE PAYLOAD ===
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

fn enforce_single_policy(
    policy_account: &AccountInfo,
    state_offset: u32,
    role_id: u32,
    slot: u64,
    execution_data: &[u8],
    config_account: &AccountInfo,
    vault_account: &AccountInfo,
    target_metas: &[AccountMeta],
    all_accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("enforce_single_policy start");
    let mut amount = 0u64;
    // Try to parse amount for System Transfer (Discriminator 2)
    if execution_data.len() >= 12 && execution_data[0] == 2 {
        let amount_bytes: [u8; 8] = execution_data[4..12].try_into().unwrap_or([0; 8]);
        amount = u64::from_le_bytes(amount_bytes);
    }

    let verify_instr = VerifyInstruction {
        discriminator: INSTRUCTION_VERIFY,
        state_offset,
        role_id,
        slot,
        amount,
        _reserved: [0; 4],
    };

    let instr_bytes = unsafe {
        core::slice::from_raw_parts(
            &verify_instr as *const VerifyInstruction as *const u8,
            VerifyInstruction::LEN,
        )
    };
    let mut instr_data_vec = instr_bytes.to_vec();
    instr_data_vec.extend_from_slice(execution_data);

    let mut meta_accounts = Vec::with_capacity(3 + target_metas.len()); // Config + Vault + Targets

    // IMPORTANT: Include policy account (executable) for CPI
    let mut invoke_accounts = Vec::with_capacity(4 + target_metas.len());
    invoke_accounts.push(policy_account);

    meta_accounts.push(AccountMeta {
        pubkey: config_account.key(),
        is_signer: false,
        is_writable: true,
    });
    invoke_accounts.push(config_account);

    meta_accounts.push(AccountMeta {
        pubkey: vault_account.key(),
        is_signer: false,
        is_writable: true,
    });
    invoke_accounts.push(vault_account);

    for target_meta in target_metas {
        let is_config = *target_meta.pubkey == *config_account.key();
        let is_vault = *target_meta.pubkey == *vault_account.key();

        if is_config || is_vault {
            continue;
        }

        meta_accounts.push(AccountMeta {
            pubkey: target_meta.pubkey,
            is_signer: false,
            is_writable: false,
        });

        if let Some(acc) = all_accounts.iter().find(|a| a.key() == target_meta.pubkey) {
            invoke_accounts.push(acc);
        }
    }

    let instruction = Instruction {
        program_id: policy_account.key(),
        accounts: &meta_accounts,
        data: &instr_data_vec,
    };

    dispatch_invoke_signed(&instruction, &invoke_accounts, &[])?;
    msg!("Policy CPI success");

    // Handle Return Data
    let mut return_data = [0u8; 128];
    let mut program_id_buf = Pubkey::default();
    let len = unsafe {
        sol_get_return_data(
            return_data.as_mut_ptr(),
            return_data.len() as u64,
            &mut program_id_buf,
        )
    };

    if len > 0 {
        if program_id_buf != *policy_account.key() {
            msg!("Return data program ID mismatch");
        } else {
            let mut config_data = config_account.try_borrow_mut_data()?;
            let offset = state_offset as usize;
            let end = offset + (len as usize);
            if end <= config_data.len() {
                config_data[offset..end].copy_from_slice(&return_data[..len as usize]);
            } else {
                return Err(ProgramError::AccountDataTooSmall);
            }
        }
    }
    Ok(())
}
