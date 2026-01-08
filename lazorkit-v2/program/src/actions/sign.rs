//! Execute instruction handler - Pure External Architecture với Plugin CPI

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_pubkey::from_str;
use lazorkit_v2_instructions::InstructionIterator;
use lazorkit_v2_state::{
    wallet_account::{wallet_account_seeds, wallet_vault_seeds_with_bump, WalletAccount, AuthorityData},
    plugin::{PluginEntry, PluginType},
    plugin_ref::PluginRef,
    AccountClassification,
    Discriminator,
    Transmutable,
    TransmutableMut,
    IntoBytes,
};

use crate::{
    error::LazorkitError,
    util::invoke::find_account_info,
};
use lazorkit_v2_assertions::check_stack_height;

pub const INSTRUCTION_SYSVAR_ACCOUNT: Pubkey =
    from_str("Sysvar1nstructions1111111111111111111111111");

/// Arguments for Execute instruction (Pure External)
#[repr(C, align(8))]
#[derive(Debug)]
pub struct ExecuteArgs {
    pub instruction: u16,  // LazorkitInstruction::Sign = 1
    pub instruction_payload_len: u16,
    pub authority_id: u32,  // Authority ID trong wallet account
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
pub fn sign(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
    account_classification: &mut [AccountClassification],
) -> ProgramResult {
    // Check stack height (security: prevent stack overflow)
    check_stack_height(1, LazorkitError::Cpi)?;
    
    if accounts.len() < 2 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }
    
    let wallet_account_info = &accounts[0];
    let wallet_vault_info = &accounts[1];
    
    // Validate WalletAccount
    let wallet_account_data = unsafe { wallet_account_info.borrow_data_unchecked() };
    if wallet_account_data.is_empty() || wallet_account_data[0] != Discriminator::WalletAccount as u8 {
        return Err(LazorkitError::InvalidWalletStateDiscriminator.into());
    }
    
    let wallet_account = unsafe {
        WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])?
    };
    
    // Parse instruction args
    if instruction_data.len() < ExecuteArgs::LEN {
        return Err(LazorkitError::InvalidSignInstructionDataTooShort.into());
    }
    
    let args = unsafe { ExecuteArgs::load_unchecked(&instruction_data[..ExecuteArgs::LEN])? };
    
    // Split instruction data
    let (instruction_payload, authority_payload) = unsafe {
        instruction_data[ExecuteArgs::LEN..]
            .split_at_unchecked(args.instruction_payload_len as usize)
    };
    
    // Get authority by ID
    let authority_data = wallet_account
        .get_authority(wallet_account_data, args.authority_id)?
        .ok_or(LazorkitError::InvalidAuthorityNotFoundByRoleId)?;
    
    // Get all plugins from registry
    let all_plugins = wallet_account.get_plugins(wallet_account_data)?;
    
    // Get enabled plugin refs for this authority (sorted by priority)
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
    )?;
    
    // Process each instruction
    for ix_result in ix_iter {
        let instruction = ix_result?;
        
        // CPI to each enabled plugin to check permission
        for plugin_ref in &enabled_refs {
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
        )?;
        
        // CPI to each enabled plugin to update state after execution
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
    
    // Update nonce
    let mut wallet_account_mut = unsafe { wallet_account_info.borrow_mut_data_unchecked() };
    let current_nonce = wallet_account.get_last_nonce(wallet_account_mut)?;
    wallet_account.set_last_nonce(wallet_account_mut, current_nonce.wrapping_add(1))?;
    
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
    cpi_data.push(1u8);  // PluginInstruction::UpdateState = 1
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
    // Format: [instruction: u8, authority_id: u32, authority_data_len: u32, authority_data, instruction_data_len: u32, instruction_data]
    let mut cpi_data = Vec::with_capacity(
        1 + 4 + 4 + authority_data.authority_data.len() + 4 + instruction.data.len()
    );
    cpi_data.push(0u8);  // PluginInstruction::CheckPermission = 0
    cpi_data.extend_from_slice(&(authority_data.position.id).to_le_bytes()); // authority_id
    cpi_data.extend_from_slice(&(authority_data.authority_data.len() as u32).to_le_bytes());
    cpi_data.extend_from_slice(&authority_data.authority_data);
    cpi_data.extend_from_slice(&(instruction.data.len() as u32).to_le_bytes());
    cpi_data.extend_from_slice(instruction.data);
    
    // CPI Accounts:
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
    
    // Use invoke_signed_dynamic like Swig
    crate::util::invoke::invoke_signed_dynamic(
        &cpi_ix,
        cpi_account_infos.as_slice(),
        &[seeds_slice],
    )?;
    
    Ok(())
}
