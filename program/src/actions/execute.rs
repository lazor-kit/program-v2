//! Execute instruction handler (Bounce Flow)

use lazorkit_interface::{VerifyInstruction, INSTRUCTION_VERIFY};
use lazorkit_state::authority::ed25519::{Ed25519Authority, Ed25519SessionAuthority};
use lazorkit_state::authority::programexec::{ProgramExecAuthority, ProgramExecSessionAuthority};
use lazorkit_state::authority::secp256k1::{Secp256k1Authority, Secp256k1SessionAuthority};
use lazorkit_state::authority::secp256r1::{Secp256r1Authority, Secp256r1SessionAuthority};
use lazorkit_state::authority::{AuthorityInfo, AuthorityType};
use lazorkit_state::{
    plugin::parse_plugins, read_position, IntoBytes, LazorKitWallet, Position, RoleIterator,
    Transmutable, TransmutableMut,
};
use pinocchio::program::invoke;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed},
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

// Return data mock removed

use crate::error::LazorKitError;

#[cfg(target_os = "solana")]
extern "C" {
    fn sol_get_return_data(data: *mut u8, length: u64, program_id: *mut Pubkey) -> u64;
}

#[cfg(not(target_os = "solana"))]
unsafe fn sol_get_return_data(_data: *mut u8, _length: u64, _program_id: *mut Pubkey) -> u64 {
    0
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
    // Other accounts are optional/variable depending on instruction
    let _system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if config_account.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // --- Phase 1: Immutable Scan ---
    let (role_found, role_position, role_abs_offset, wallet_bump) = {
        let config_data = config_account.try_borrow_data()?;
        let wallet =
            unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN])? };
        let wallet_bump_val = wallet.wallet_bump;

        let mut found = false;
        let mut position: Option<Position> = None;
        let mut offset = 0;
        let mut current_cursor = LazorKitWallet::LEN;

        // Iterate manually to find offset
        // RoleIterator returns relative slices, we need absolute offset for later mutable access.
        // Actually RoleIterator logic is: start at cursor, read position, jump to boundary.

        let mut remaining = wallet.role_count;
        while remaining > 0 {
            if current_cursor + Position::LEN > config_data.len() {
                break;
            }
            // Using read_position helper which uses unsafe load_unchecked
            let pos_ref = read_position(&config_data[current_cursor..])?;

            if pos_ref.id == role_id {
                found = true;
                position = Some(*pos_ref);
                offset = current_cursor;
                break;
            }

            current_cursor = pos_ref.boundary as usize;
            remaining -= 1;
        }
        (found, position, offset, wallet_bump_val)
    };

    if !role_found {
        msg!("Role {} not found", role_id);
        return Err(LazorKitError::AuthorityNotFound.into());
    }

    let pos = role_position.unwrap();
    msg!(
        "Role found. Type: {}, Len: {}",
        pos.authority_type,
        pos.authority_length
    );
    let slot = Clock::get()?.slot;

    // Payload format: [signer_index(1), instruction_data...]
    if instruction_payload.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (auth_payload, execution_data) = instruction_payload.split_at(1);

    // --- Phase 2: Mutable Process ---
    let mut config_data = config_account.try_borrow_mut_data()?;

    // Auth Data Slice (Mutable)
    // Auth Data Bounds
    let auth_start = role_abs_offset + Position::LEN;
    let auth_end = auth_start + pos.authority_length as usize;
    if auth_end > config_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Plugins Data Bounds
    let plugins_start = auth_end;
    let plugins_end = pos.boundary as usize;
    msg!(
        "Debug: plugins_start={}, plugins_end={}, boundary={}",
        plugins_start,
        plugins_end,
        pos.boundary
    );
    if plugins_end > config_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }

    // === 1. AUTHENTICATION ===
    let mut exclude_signer_index: Option<usize> = None;
    {
        let mut authority_data_slice = &mut config_data[auth_start..auth_end];
        let auth_type = AuthorityType::try_from(pos.authority_type)?;
        msg!("Auth Type: {:?}", auth_type);

        if matches!(
            auth_type,
            AuthorityType::Ed25519 | AuthorityType::Ed25519Session
        ) {
            if let Some(&idx) = auth_payload.first() {
                exclude_signer_index = Some(idx as usize);
            }
        }

        match auth_type {
            AuthorityType::Ed25519 => {
                let mut auth =
                    unsafe { Ed25519Authority::load_mut_unchecked(authority_data_slice) }
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            },
            AuthorityType::Ed25519Session => {
                let mut auth =
                    unsafe { Ed25519SessionAuthority::load_mut_unchecked(authority_data_slice) }
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            },
            AuthorityType::Secp256k1 => {
                let mut auth =
                    unsafe { Secp256k1Authority::load_mut_unchecked(authority_data_slice) }
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            },
            AuthorityType::Secp256k1Session => {
                let mut auth =
                    unsafe { Secp256k1SessionAuthority::load_mut_unchecked(authority_data_slice) }
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            },
            AuthorityType::Secp256r1 => {
                let mut auth =
                    unsafe { Secp256r1Authority::load_mut_unchecked(authority_data_slice) }
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            },
            AuthorityType::Secp256r1Session => {
                let mut auth =
                    unsafe { Secp256r1SessionAuthority::load_mut_unchecked(authority_data_slice) }
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            },
            AuthorityType::ProgramExec => {
                let mut auth =
                    unsafe { ProgramExecAuthority::load_mut_unchecked(authority_data_slice) }
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            },
            AuthorityType::ProgramExecSession => {
                let mut auth = unsafe {
                    ProgramExecSessionAuthority::load_mut_unchecked(authority_data_slice)
                }
                .map_err(|_| ProgramError::InvalidAccountData)?;
                auth.authenticate(accounts, auth_payload, execution_data, slot)?;
            },
            AuthorityType::None => {
                return Err(ProgramError::InvalidInstructionData);
            },
        }
    } // End Authentication Block

    msg!(
        "Execute: role={}, plugins={}, payload_len={}",
        role_id,
        pos.num_actions,
        instruction_payload.len()
    );

    // === 2. BOUNCE FLOW: Iterate through plugins ===

    // === 2. BOUNCE FLOW: Iterate through plugins ===
    // Calculate start offset of plugins data for absolute offset calculation
    let plugins_start_offset = role_abs_offset + Position::LEN + pos.authority_length as usize;
    let plugins_end_offset = pos.boundary as usize;

    use alloc::vec::Vec; // Import Vec from alloc

    // Collect plugin info to avoid holding mutable borrow during CPI
    let plugin_cpi_infos = {
        let plugins_slice = &config_data[plugins_start_offset..plugins_end_offset];
        let mut infos = Vec::new(); // Use Vec directly
        for plugin_result in parse_plugins(plugins_slice) {
            let plugin_view = plugin_result.map_err(|_| ProgramError::InvalidAccountData)?;
            let pid = plugin_view.header.program_id();
            // State blob follows header
            let state_offset = plugins_start_offset
                + plugin_view.offset
                + lazorkit_state::plugin::PluginHeader::LEN;
            infos.push((pid, state_offset as u32));
        }
        infos
    };

    // Drop mutable borrow of config_data to allow plugins to borrow it
    drop(config_data);

    let mut plugin_found = false;

    for (plugin_program_id, state_offset) in &plugin_cpi_infos {
        // Find the plugin account in 'accounts'
        // We need the AccountInfo corresponding to plugin_program_id.
        // It should be passed in `accounts`.
        // TODO: This linear search might be costly if many accounts.
        let plugin_account = accounts.iter().find(|a| a.key() == plugin_program_id);

        if let Some(acc) = plugin_account {
            // Found executable plugin account
            plugin_found = true;

            let verify_instr = VerifyInstruction {
                discriminator: INSTRUCTION_VERIFY,
                state_offset: *state_offset,
                role_id: pos.id,
                slot,
                amount: 0,
                _reserved: [0; 4],
            };

            let instr_bytes = unsafe {
                core::slice::from_raw_parts(
                    &verify_instr as *const VerifyInstruction as *const u8,
                    VerifyInstruction::LEN,
                )
            };

            let metas = vec![
                AccountMeta {
                    pubkey: config_account.key(),
                    is_signer: false,
                    is_writable: true,
                }, // Config (Mutable)
                AccountMeta {
                    pubkey: vault_account.key(),
                    is_signer: false,
                    is_writable: true,
                }, // Vault (Mutable) (Source of funds?)
                   // Pass the plugin account itself? Not needed for CPI usually unless it's data
            ];
            let instruction = Instruction {
                program_id: plugin_program_id,
                accounts: &metas,
                data: instr_bytes,
            };

            // ... (existing invoke logic) ...
            // Invoke plugin. It will update state in-place if successful.
            invoke(&instruction, &[config_account, vault_account, acc])?;

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
                // Verify program_id matches plugin_program_id
                if program_id_buf != *plugin_program_id {
                    // Warning but maybe not error if some inner CPI did it?
                    msg!("Return data program ID mismatch: {:?}", program_id_buf);
                } else {
                    // Apply return data to state
                    let mut config_data = config_account.try_borrow_mut_data()?;
                    let offset = *state_offset as usize;
                    let end = offset + (len as usize);

                    // Bounds check matches SolLimtState::LEN ideally
                    if end <= config_data.len() {
                        config_data[offset..end].copy_from_slice(&return_data[..len as usize]);
                        msg!("Applied return data from plugin via syscall");
                    } else {
                        msg!("Return data write out of bounds");
                        return Err(ProgramError::AccountDataTooSmall);
                    }
                }
            }

            break;
        }
    }

    if !plugin_found {
        msg!("Plugin account not provided");
        return Err(ProgramError::InvalidArgument);
    }

    // No return data processing needed - state is updated in-place

    msg!("All plugin verifications passed");

    // === 3. EXECUTE PAYLOAD ===
    if accounts.len() < 4 {
        msg!("Missing target program account");
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let target_program = &accounts[3];
    let target_instruction_data = execution_data.to_vec();

    // Construct AccountMetas for target instruction
    let mut target_account_metas = vec![];
    msg!("Debug: Starting loop. accounts.len={}", accounts.len());
    // Start from index 4 (TargetAccount1)
    for (i, acc) in accounts[4..].iter().enumerate() {
        let abs_index = 4 + i;
        if Some(abs_index) == exclude_signer_index {
            continue;
        }

        // Filter out Plugin Accounts (Executables used in CPI)
        if plugin_cpi_infos.iter().any(|(pid, _)| pid == acc.key()) {
            continue;
        }

        let mut meta = AccountMeta {
            pubkey: acc.key(),
            is_signer: acc.is_signer(),
            is_writable: acc.is_writable(),
        };
        // If account matches Vault, force is_signer=true (for CPI)
        if acc.key() == vault_account.key() {
            meta.is_signer = true;
        }

        target_account_metas.push(meta);
    }

    // Invoke signed
    // Invoke signed
    let execute_instruction = Instruction {
        program_id: target_program.key(),
        accounts: &target_account_metas,
        data: &target_instruction_data,
    };

    let seeds = &[
        b"lazorkit-wallet-address",
        config_account.key().as_ref(), // This matches expected_config in create_wallet
        &[wallet_bump], // This matches vault_bump_arr in create_wallet (variable name mismatch implies logic)
    ];
    let signer_seeds = &[&seeds[..]];

    let seed_list = [
        Seed::from(seeds[0]),
        Seed::from(seeds[1]),
        Seed::from(seeds[2]),
    ];
    let signer = pinocchio::instruction::Signer::from(&seed_list);
    let signers = [signer];

    // Dynamic invoke loop up to 16 accounts to satisfy Pinocchio's array requirement
    match accounts.len() {
        1 => invoke_signed(&execute_instruction, &[&accounts[0]], &signers)?,
        2 => invoke_signed(
            &execute_instruction,
            &[&accounts[0], &accounts[1]],
            &signers,
        )?,
        3 => invoke_signed(
            &execute_instruction,
            &[&accounts[0], &accounts[1], &accounts[2]],
            &signers,
        )?,
        4 => invoke_signed(
            &execute_instruction,
            &[&accounts[0], &accounts[1], &accounts[2], &accounts[3]],
            &signers,
        )?,
        5 => invoke_signed(
            &execute_instruction,
            &[
                &accounts[0],
                &accounts[1],
                &accounts[2],
                &accounts[3],
                &accounts[4],
            ],
            &signers,
        )?,
        6 => invoke_signed(
            &execute_instruction,
            &[
                &accounts[0],
                &accounts[1],
                &accounts[2],
                &accounts[3],
                &accounts[4],
                &accounts[5],
            ],
            &signers,
        )?,
        7 => invoke_signed(
            &execute_instruction,
            &[
                &accounts[0],
                &accounts[1],
                &accounts[2],
                &accounts[3],
                &accounts[4],
                &accounts[5],
                &accounts[6],
            ],
            &signers,
        )?,
        8 => invoke_signed(
            &execute_instruction,
            &[
                &accounts[0],
                &accounts[1],
                &accounts[2],
                &accounts[3],
                &accounts[4],
                &accounts[5],
                &accounts[6],
                &accounts[7],
            ],
            &signers,
        )?,
        9 => invoke_signed(
            &execute_instruction,
            &[
                &accounts[0],
                &accounts[1],
                &accounts[2],
                &accounts[3],
                &accounts[4],
                &accounts[5],
                &accounts[6],
                &accounts[7],
                &accounts[8],
            ],
            &signers,
        )?,
        10 => invoke_signed(
            &execute_instruction,
            &[
                &accounts[0],
                &accounts[1],
                &accounts[2],
                &accounts[3],
                &accounts[4],
                &accounts[5],
                &accounts[6],
                &accounts[7],
                &accounts[8],
                &accounts[9],
            ],
            &signers,
        )?,
        11 => invoke_signed(
            &execute_instruction,
            &[
                &accounts[0],
                &accounts[1],
                &accounts[2],
                &accounts[3],
                &accounts[4],
                &accounts[5],
                &accounts[6],
                &accounts[7],
                &accounts[8],
                &accounts[9],
                &accounts[10],
            ],
            &signers,
        )?,
        12 => invoke_signed(
            &execute_instruction,
            &[
                &accounts[0],
                &accounts[1],
                &accounts[2],
                &accounts[3],
                &accounts[4],
                &accounts[5],
                &accounts[6],
                &accounts[7],
                &accounts[8],
                &accounts[9],
                &accounts[10],
                &accounts[11],
            ],
            &signers,
        )?,
        _ => return Err(ProgramError::AccountDataTooSmall), // Limit for now, expand if needed
    }

    msg!("Execute completed for role {}", role_id);
    Ok(())
}

// Return data capture removed as it is unused and caused link errors
