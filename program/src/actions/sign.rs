//! Execute instruction handler - Pure External Architecture với Plugin CPI

use lazorkit_v2_instructions::InstructionIterator;
use lazorkit_v2_state::{
    plugin::PluginEntry,
    plugin_ref::PluginRef,
    wallet_account::{
        wallet_account_seeds, wallet_vault_seeds_with_bump, AuthorityData, WalletAccount,
    },
    Discriminator, IntoBytes, Transmutable, TransmutableMut,
};
use pinocchio::msg;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_pubkey::from_str;

use crate::{
    error::LazorkitError,
    util::invoke::find_account_info,
    util::snapshot::{capture_account_snapshot, hash_except, verify_account_snapshot},
};
use core::mem::MaybeUninit;
use lazorkit_v2_assertions::check_stack_height;

pub const INSTRUCTION_SYSVAR_ACCOUNT: Pubkey =
    from_str("Sysvar1nstructions1111111111111111111111111");

/// Arguments for Execute instruction (Pure External)
/// Note: instruction discriminator is already parsed in process_action, so we only have:
/// - instruction_payload_len: u16 (2 bytes)
/// - authority_id: u32 (4 bytes)
/// Total: 6 bytes, but aligned to 8 bytes
#[repr(C, align(8))]
#[derive(Debug)]
pub struct ExecuteArgs {
    pub instruction_payload_len: u16, // 2 bytes
    pub authority_id: u32,            // 4 bytes
                                      // Padding to 8 bytes alignment (2 bytes implicit)
}

impl ExecuteArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for ExecuteArgs {
    const LEN: usize = Self::LEN;
}

/// Executes a transaction with plugin permission checks (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. wallet_vault (signer, system-owned PDA)
/// 2..N. Other accounts for inner instructions
pub fn sign(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    // Check stack height (security: prevent stack overflow)
    check_stack_height(1, LazorkitError::Cpi)?;

    if accounts.len() < 2 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }

    let wallet_account_info = &accounts[0];
    let wallet_vault_info = &accounts[1];

    // Validate WalletAccount
    let wallet_account_data = unsafe { wallet_account_info.borrow_data_unchecked() };
    if wallet_account_data.is_empty()
        || wallet_account_data[0] != Discriminator::WalletAccount as u8
    {
        return Err(LazorkitError::InvalidWalletStateDiscriminator.into());
    }

    let wallet_account =
        unsafe { WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])? };

    // Parse instruction args
    if instruction_data.len() < ExecuteArgs::LEN {
        return Err(LazorkitError::InvalidSignInstructionDataTooShort.into());
    }

    // Parse manually (ExecuteArgs has alignment issues)
    if instruction_data.len() < 6 {
        return Err(LazorkitError::InvalidSignInstructionDataTooShort.into());
    }

    let instruction_payload_len = u16::from_le_bytes([instruction_data[0], instruction_data[1]]);
    let authority_id = u32::from_le_bytes([
        instruction_data[2],
        instruction_data[3],
        instruction_data[4],
        instruction_data[5],
    ]);
    // Split instruction data
    // Format after process_action: [payload_len: u16, authority_id: u32, instruction_payload, authority_payload]
    // Actual data is 6 bytes (2 + 4), not 8 bytes (ExecuteArgs struct has padding but data doesn't)
    let args_offset = 6; // payload_len (2) + authority_id (4) = 6 bytes
    let available_after_offset = instruction_data.len().saturating_sub(args_offset);

    // Use available data if payload_len is larger than available (defensive)
    let actual_payload_len =
        core::cmp::min(instruction_payload_len as usize, available_after_offset);

    let instruction_payload = &instruction_data[args_offset..args_offset + actual_payload_len];
    let authority_payload = &instruction_data[args_offset + actual_payload_len..];

    // Get authority by ID
    let authority_data = wallet_account
        .get_authority(wallet_account_data, authority_id)?
        .ok_or_else(|| LazorkitError::InvalidAuthorityNotFoundByRoleId)?;

    // Only All and AllButManageAuthority can bypass CPI plugin checks
    // ExecuteOnly must check plugins, ManageAuthority cannot execute (error)
    let role_perm_byte = authority_data.position.role_permission;
    let role_perm = authority_data.position.role_permission().map_err(|e| e)?;
    let (has_all_permission, should_skip_plugin_checks) =
        crate::util::permission::check_role_permission_for_execute(&authority_data)
            .map_err(|e| e)?;

    // Get all plugins from registry (only needed if we need to check plugins)

    let all_plugins = wallet_account
        .get_plugins(wallet_account_data)
        .map_err(|e| e)?;

    // Get enabled plugin refs for this authority (sorted by priority)
    // Only used if we're checking plugins (ExecuteOnly)

    let mut enabled_refs: Vec<&PluginRef> = authority_data
        .plugin_refs
        .iter()
        .filter(|r| r.is_enabled())
        .collect();
    enabled_refs.sort_by_key(|r| r.priority);

    // Prepare wallet vault signer seeds
    // Wallet vault is derived from wallet_account key (not id)
    let wallet_bump = [wallet_account.wallet_bump];
    let wallet_vault_seeds: [Seed; 3] = [
        Seed::from(WalletAccount::WALLET_VAULT_SEED),
        Seed::from(wallet_account_info.key().as_ref()),
        Seed::from(wallet_bump.as_ref()),
    ];

    // Parse embedded instructions
    let rkeys: &[&Pubkey] = &[];
    let ix_iter = InstructionIterator::new(
        accounts,
        instruction_payload,
        wallet_vault_info.key(),
        rkeys,
    )
    .map_err(|e| e)?;

    // ACCOUNT SNAPSHOTS: Capture account state BEFORE instruction execution
    // This ensures accounts aren't modified unexpectedly by malicious instructions
    const UNINIT_HASH: MaybeUninit<[u8; 32]> = MaybeUninit::uninit();
    let mut account_snapshots: [MaybeUninit<[u8; 32]>; 100] = [UNINIT_HASH; 100];
    let mut snapshot_captured: [bool; 100] = [false; 100]; // Track which accounts have snapshots
    const NO_EXCLUDE_RANGES: &[core::ops::Range<usize>] = &[];

    for (index, account) in accounts.iter().enumerate() {
        if index >= 100 {
            break; // Limit to 100 accounts
        }

        // Only snapshot writable accounts (read-only accounts won't be modified)
        if let Some(hash) = capture_account_snapshot(account, NO_EXCLUDE_RANGES) {
            account_snapshots[index].write(hash);
            snapshot_captured[index] = true;
        }
    }

    // Process each instruction
    let mut ix_idx = 0;
    for ix_result in ix_iter {
        let instruction = ix_result.map_err(|e| e)?;

        // CPI to each enabled plugin to check permission
        // Only check if not bypassing (ExecuteOnly needs to check plugins)
        if !should_skip_plugin_checks {
            for plugin_ref in &enabled_refs {
                if (plugin_ref.plugin_index as usize) >= all_plugins.len() {
                    return Err(LazorkitError::PluginNotFound.into());
                }

                let plugin = &all_plugins[plugin_ref.plugin_index as usize];

                check_plugin_permission(
                    plugin,
                    &instruction,
                    accounts,
                    wallet_account_info,
                    wallet_vault_info,
                    &authority_data,
                    &wallet_vault_seeds[..],
                )?;
            }

            // CPI SECURITY: Check program whitelist if wallet_vault is signer
            // This prevents malicious plugins from calling unauthorized programs
            // Only check when NOT bypassing plugins (i.e., not All permission)
            let wallet_vault_is_signer = instruction
                .accounts
                .iter()
                .any(|meta| meta.pubkey == wallet_vault_info.key() && meta.is_signer);

            if wallet_vault_is_signer {
                // Whitelist of safe programs
                // Note: instruction.program_id is &[u8; 32], so we compare as byte arrays
                let is_allowed =
                    instruction.program_id == solana_program::system_program::ID.as_ref();
                // TODO: Add Token programs when dependencies are available
                // || instruction.program_id == spl_token::ID.as_ref()
                // || instruction.program_id == spl_token_2022::ID.as_ref();

                if !is_allowed {
                    return Err(LazorkitError::UnauthorizedCpiProgram.into());
                }
            }
        } else {
        }

        // Execute instruction using invoke_signed_dynamic
        // Map instruction accounts to AccountInfos
        let mut instruction_account_infos = Vec::with_capacity(instruction.accounts.len());
        for meta in instruction.accounts {
            instruction_account_infos.push(find_account_info(meta.pubkey, accounts)?);
        }

        // Convert Seed array to &[&[u8]] for invoke_signed_dynamic
        let seeds_refs: Vec<&[u8]> = wallet_vault_seeds
            .iter()
            .map(|s| unsafe { *(s as *const _ as *const &[u8]) })
            .collect();
        let seeds_slice = seeds_refs.as_slice();

        // Create Instruction struct
        let instruction_struct = Instruction {
            program_id: instruction.program_id,
            accounts: instruction.accounts,
            data: instruction.data,
        };

        // Invoke instruction
        crate::util::invoke::invoke_signed_dynamic(
            &instruction_struct,
            instruction_account_infos.as_slice(),
            &[seeds_slice],
        )
        .map_err(|e| e)?;

        // ACCOUNT SNAPSHOTS: Verify accounts weren't modified unexpectedly
        // Only verify accounts that we captured snapshots for (writable accounts)
        // Only verify accounts that we captured snapshots for (writable accounts)
        for (index, account) in accounts.iter().enumerate() {
            if index >= 100 {
                break;
            }

            // Only verify if we captured a snapshot for this account
            if snapshot_captured[index] {
                let snapshot_hash = unsafe { account_snapshots[index].assume_init_ref() };
                verify_account_snapshot(account, snapshot_hash, NO_EXCLUDE_RANGES)
                    .map_err(|e| e)?;
            }
        }

        ix_idx += 1;

        // CPI to each enabled plugin to update state after execution
        // Only update if not bypassing (ExecuteOnly needs to update plugin state)
        if !should_skip_plugin_checks {
            for plugin_ref in &enabled_refs {
                let plugin = &all_plugins[plugin_ref.plugin_index as usize];

                update_plugin_state(
                    plugin,
                    &instruction,
                    accounts,
                    wallet_account_info,
                    wallet_vault_info,
                    &wallet_vault_seeds[..],
                )?;
            }
        }
    }

    // RENT EXEMPTION CHECK: Ensure wallet_vault and wallet_account have enough balance
    // This prevents the wallet from being closed due to insufficient rent

    let wallet_vault_data = wallet_vault_info.try_borrow_data()?;
    let rent = pinocchio::sysvars::rent::Rent::get()?;
    let rent_exempt_minimum = rent.minimum_balance(wallet_vault_data.len());
    let current_balance = wallet_vault_info.lamports();

    if current_balance < rent_exempt_minimum {
        return Err(LazorkitError::InsufficientBalance.into());
    }

    // Also check wallet_account
    let wallet_account_data_len = wallet_account_info.data_len();
    let wallet_account_rent_min = rent.minimum_balance(wallet_account_data_len);
    let wallet_account_balance = wallet_account_info.lamports();

    if wallet_account_balance < wallet_account_rent_min {
        return Err(LazorkitError::InsufficientBalance.into());
    }

    // Note: Nonce is not used. Each authority has its own odometer for replay protection.
    // Odometer is updated in the authority's authenticate() method.

    Ok(())
}

/// Update plugin state via CPI after instruction execution (Pure External architecture)
fn update_plugin_state(
    plugin: &PluginEntry,
    instruction: &lazorkit_v2_instructions::InstructionHolder,
    all_accounts: &[AccountInfo],
    wallet_account_info: &AccountInfo,
    wallet_vault_info: &AccountInfo,
    signer_seeds: &[Seed],
) -> ProgramResult {
    // Construct CPI instruction data for plugin state update
    // Format: [instruction: u8, instruction_data_len: u32, instruction_data]
    let mut cpi_data = Vec::with_capacity(1 + 4 + instruction.data.len());
    cpi_data.push(2u8); // PluginInstruction::UpdateConfig = 2 (for sol-limit plugin)
    cpi_data.extend_from_slice(&(instruction.data.len() as u32).to_le_bytes());
    cpi_data.extend_from_slice(instruction.data);

    // CPI Accounts:
    // [0] Plugin Config PDA (writable)
    // [1] Wallet Account (read-only, for plugin to read wallet state)
    // [2] Wallet Vault (signer - proves authorized call)
    // [3..] Instruction accounts (for plugin to update state based on execution)
    let mut cpi_accounts = Vec::with_capacity(3 + instruction.accounts.len());
    cpi_accounts.push(AccountMeta {
        pubkey: &plugin.config_account,
        is_signer: false,
        is_writable: true,
    });
    cpi_accounts.push(AccountMeta {
        pubkey: wallet_account_info.key(),
        is_signer: false,
        is_writable: false,
    });
    cpi_accounts.push(AccountMeta {
        pubkey: wallet_vault_info.key(),
        is_signer: true,
        is_writable: false,
    });

    // Map instruction accounts to AccountMeta
    for meta in instruction.accounts {
        cpi_accounts.push(AccountMeta {
            pubkey: meta.pubkey,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        });
    }

    // Map AccountMeta to AccountInfo for CPI
    let mut cpi_account_infos = Vec::new();
    for meta in &cpi_accounts {
        cpi_account_infos.push(find_account_info(meta.pubkey, all_accounts)?);
    }

    // CPI to plugin program
    let cpi_ix = Instruction {
        program_id: &plugin.program_id,
        accounts: &cpi_accounts,
        data: &cpi_data,
    };

    // Convert Seed array to &[&[u8]] for invoke_signed_dynamic
    let seeds_refs: Vec<&[u8]> = signer_seeds
        .iter()
        .map(|s| unsafe { *(s as *const _ as *const &[u8]) })
        .collect();
    let seeds_slice = seeds_refs.as_slice();

    // Invoke plugin update state
    crate::util::invoke::invoke_signed_dynamic(
        &cpi_ix,
        cpi_account_infos.as_slice(),
        &[seeds_slice],
    )?;

    Ok(())
}

/// Check plugin permission via CPI (Pure External architecture)
fn check_plugin_permission(
    plugin: &PluginEntry,
    instruction: &lazorkit_v2_instructions::InstructionHolder,
    all_accounts: &[AccountInfo],
    wallet_account_info: &AccountInfo,
    wallet_vault_info: &AccountInfo,
    authority_data: &AuthorityData,
    signer_seeds: &[Seed],
) -> ProgramResult {
    // Construct CPI instruction data for plugin
    // Format: [instruction: u8, authority_id: u32, authority_data_len: u32, authority_data, program_id: 32 bytes, instruction_data_len: u32, instruction_data]
    let mut cpi_data = Vec::with_capacity(
        1 + 4 + 4 + authority_data.authority_data.len() + 32 + 4 + instruction.data.len(),
    );
    cpi_data.push(0u8); // PluginInstruction::CheckPermission = 0
    cpi_data.extend_from_slice(&(authority_data.position.id).to_le_bytes()); // authority_id
    cpi_data.extend_from_slice(&(authority_data.authority_data.len() as u32).to_le_bytes());
    cpi_data.extend_from_slice(&authority_data.authority_data);
    cpi_data.extend_from_slice(instruction.program_id.as_ref()); // program_id (32 bytes)
    cpi_data.extend_from_slice(&(instruction.data.len() as u32).to_le_bytes());
    cpi_data.extend_from_slice(instruction.data);

    // [0] Plugin Config PDA (writable)
    // [1] Wallet Account (read-only, for plugin to read wallet state)
    // [2] Wallet Vault (signer - proves authorized call)
    // [3..] Instruction accounts (for plugin inspection)
    let mut cpi_accounts = Vec::with_capacity(3 + instruction.accounts.len());
    cpi_accounts.push(AccountMeta {
        pubkey: &plugin.config_account,
        is_signer: false,
        is_writable: true,
    });
    cpi_accounts.push(AccountMeta {
        pubkey: wallet_account_info.key(),
        is_signer: false,
        is_writable: false,
    });
    cpi_accounts.push(AccountMeta {
        pubkey: wallet_vault_info.key(),
        is_signer: true,
        is_writable: false,
    });

    // Map instruction accounts to AccountMeta
    for meta in instruction.accounts {
        cpi_accounts.push(AccountMeta {
            pubkey: meta.pubkey,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        });
    }

    // Map AccountMeta to AccountInfo for CPI
    let mut cpi_account_infos = Vec::new();
    for (idx, meta) in cpi_accounts.iter().enumerate() {
        match find_account_info(meta.pubkey, all_accounts) {
            Ok(acc) => {
                cpi_account_infos.push(acc);
            },
            Err(e) => {
                return Err(e);
            },
        }
    }

    // CPI to plugin program
    let cpi_ix = Instruction {
        program_id: &plugin.program_id,
        accounts: &cpi_accounts,
        data: &cpi_data,
    };

    // Convert Seed array to &[&[u8]] for invoke_signed_dynamic
    let seeds_refs: Vec<&[u8]> = signer_seeds
        .iter()
        .map(|s| unsafe { *(s as *const _ as *const &[u8]) })
        .collect();
    let seeds_slice = seeds_refs.as_slice();

    // Use invoke_signed_dynamic
    let cpi_result = crate::util::invoke::invoke_signed_dynamic(
        &cpi_ix,
        cpi_account_infos.as_slice(),
        &[seeds_slice],
    );

    match &cpi_result {
        Ok(_) => {},
        Err(e) => {
            return Err(*e);
        },
    }

    cpi_result?;

    Ok(())
}
