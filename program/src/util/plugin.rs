//! Plugin permission checking utilities

use crate::util::invoke::find_account_info;
use lazorkit_v2_state::{plugin::PluginEntry, wallet_account::AuthorityData};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed},
    program_error::ProgramError,
    ProgramResult,
};

/// Check plugin authorization/permission via CPI for instruction data
///
/// **Key Point:** This is AUTHORIZATION check, not authentication.
/// - Authentication (signature verification) should be done before calling this
/// - Authorization/permission check is done here by plugins via CPI
/// - Lazorkit V2 does NOT know what permissions each authority has
/// - Only the plugin knows and enforces the permission rules
///
/// This version works with raw instruction data (for actions like add_authority, remove_authority, etc.)
pub fn check_plugin_permission_for_instruction_data(
    plugin: &PluginEntry,
    authority_data: &AuthorityData,
    instruction_data: &[u8],
    all_accounts: &[AccountInfo],
    wallet_account_info: &AccountInfo,
    wallet_vault_info: Option<&AccountInfo>,
) -> ProgramResult {
    // Construct CPI instruction data for plugin
    // Format: [instruction: u8, authority_id: u32, authority_data_len: u32, authority_data, instruction_data_len: u32, instruction_data]
    let mut cpi_data = Vec::with_capacity(
        1 + 4 + 4 + authority_data.authority_data.len() + 4 + instruction_data.len(),
    );
    cpi_data.push(0u8); // PluginInstruction::CheckPermission = 0
    cpi_data.extend_from_slice(&(authority_data.position.id).to_le_bytes()); // authority_id
    cpi_data.extend_from_slice(&(authority_data.authority_data.len() as u32).to_le_bytes());
    cpi_data.extend_from_slice(&authority_data.authority_data);
    cpi_data.extend_from_slice(&(instruction_data.len() as u32).to_le_bytes());
    cpi_data.extend_from_slice(instruction_data);

    // CPI Accounts:
    // [0] Plugin Config PDA (writable)
    // [1] Wallet Account (read-only, for plugin to read wallet state)
    // [2] Wallet Vault (signer - optional, for actions that don't have wallet_vault)
    let mut cpi_accounts = Vec::with_capacity(3);
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

    // Add wallet_vault if provided (for sign instruction)
    // For other actions (add_authority, remove_authority, etc.), wallet_vault is not needed
    if let Some(wallet_vault) = wallet_vault_info {
        cpi_accounts.push(AccountMeta {
            pubkey: wallet_vault.key(),
            is_signer: true,
            is_writable: false,
        });
    } else {
        // Use wallet_account as placeholder (plugin should validate based on instruction discriminator)
        cpi_accounts.push(AccountMeta {
            pubkey: wallet_account_info.key(),
            is_signer: false,
            is_writable: false,
        });
    }

    // Map AccountMeta to AccountInfo for CPI
    let mut cpi_account_infos = Vec::new();
    for (i, meta) in cpi_accounts.iter().enumerate() {
        let account_info = find_account_info(meta.pubkey, all_accounts)?;
        cpi_account_infos.push(account_info);
    }

    // CPI to plugin program
    let cpi_ix = Instruction {
        program_id: &plugin.program_id,
        accounts: &cpi_accounts,
        data: &cpi_data,
    };

    // Invoke plugin permission check (no signer seeds needed for these actions)
    crate::util::invoke::invoke_signed_dynamic(
        &cpi_ix,
        cpi_account_infos.as_slice(),
        &[], // No signer seeds needed
    )?;

    Ok(())
}
