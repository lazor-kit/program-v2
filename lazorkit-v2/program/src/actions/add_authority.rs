//! Add Authority instruction handler - Pure External Architecture

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;
use lazorkit_v2_assertions::{check_self_owned, check_system_owner};
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    position::Position,
    plugin_ref::PluginRef,
    plugin::{PluginEntry, PluginType},
    authority::AuthorityType,
    Discriminator,
    Transmutable,
    TransmutableMut,
    IntoBytes,
};

use crate::error::LazorkitError;
use crate::util::invoke::find_account_info;

/// Arguments for AddAuthority instruction (Pure External)
/// Note: instruction discriminator is already parsed in process_action
#[repr(C, align(8))]
#[derive(Debug)]
pub struct AddAuthorityArgs {
    pub new_authority_type: u16,
    pub new_authority_data_len: u16,
    pub num_plugin_refs: u16,  // Number of plugin refs (usually 0 initially)
    pub _padding: [u8; 2],
}

impl AddAuthorityArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for AddAuthorityArgs {
    const LEN: usize = Self::LEN;
}

/// Adds a new authority to the wallet (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. payer (writable, signer)
/// 2. system_program
pub fn add_authority(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 3 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }
    
    let wallet_account_info = &accounts[0];
    let payer = &accounts[1];
    let system_program = &accounts[2];
    
    // Validate system program
    if system_program.key() != &pinocchio_system::ID {
        return Err(LazorkitError::InvalidSystemProgram.into());
    }
    
    // Validate wallet account
    check_self_owned(wallet_account_info, LazorkitError::OwnerMismatchWalletState)?;
    
    let wallet_account_data = unsafe { wallet_account_info.borrow_data_unchecked() };
    if wallet_account_data.is_empty() || wallet_account_data[0] != Discriminator::WalletAccount as u8 {
        return Err(LazorkitError::InvalidWalletStateDiscriminator.into());
    }
    
    let wallet_account = unsafe {
        WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])?
    };
    
    // Parse instruction args
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    // So instruction_data here starts after the discriminator
    if instruction_data.len() < AddAuthorityArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    // Parse fields manually to avoid alignment issues
    // AddAuthorityArgs: new_authority_type (2) + new_authority_data_len (2) + num_plugin_refs (2) + padding (2) = 8 bytes
    let new_authority_type = u16::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
    ]);
    let new_authority_data_len = u16::from_le_bytes([
        instruction_data[2],
        instruction_data[3],
    ]);
    let num_plugin_refs = u16::from_le_bytes([
        instruction_data[4],
        instruction_data[5],
    ]);
    // padding at [6..8] - ignore
    
    // Parse authority data
    let authority_data_start = AddAuthorityArgs::LEN;
    let authority_data_end = authority_data_start + new_authority_data_len as usize;
    
    if instruction_data.len() < authority_data_end {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let authority_data = &instruction_data[authority_data_start..authority_data_end];
    
    // Parse plugin refs (if any)
    let plugin_refs_start = authority_data_end;
    let plugin_refs_end = plugin_refs_start + (num_plugin_refs as usize * PluginRef::LEN);
    if instruction_data.len() < plugin_refs_end {
        return Err(ProgramError::InvalidInstructionData);
    }
    let plugin_refs_data = &instruction_data[plugin_refs_start..plugin_refs_end];
    
    // Validate authority type
    let authority_type = AuthorityType::try_from(new_authority_type)
        .map_err(|_| LazorkitError::InvalidAuthorityType)?;
    
    // CPI to role/permission plugin to validate add authority (if plugin exists)
    // Find role/permission plugin in registry
    let all_plugins = wallet_account.get_plugins(wallet_account_data)?;
    let role_permission_plugin = all_plugins
        .iter()
        .find(|p| p.plugin_type() == PluginType::RolePermission && p.is_enabled());
    
    if let Some(plugin) = role_permission_plugin {
        // Build CPI instruction data for validation
        // Format: [instruction: u8, authority_data_len: u32, authority_data, num_plugin_refs: u16, plugin_refs]
        let mut cpi_data = Vec::with_capacity(
            1 + 4 + authority_data.len() + 2 + plugin_refs_data.len()
        );
        cpi_data.push(2u8);  // PluginInstruction::ValidateAddAuthority = 2
        cpi_data.extend_from_slice(&(authority_data.len() as u32).to_le_bytes());
        cpi_data.extend_from_slice(authority_data);
        cpi_data.extend_from_slice(&num_plugin_refs.to_le_bytes());
        cpi_data.extend_from_slice(plugin_refs_data);
        
        // CPI Accounts:
        // [0] Plugin Config PDA (writable)
        // [1] Wallet Account (read-only, for plugin to read wallet state)
        let mut cpi_accounts = Vec::with_capacity(2);
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
        
        // Map AccountMeta to AccountInfo for CPI
        let mut cpi_account_infos = Vec::new();
        for meta in &cpi_accounts {
            cpi_account_infos.push(find_account_info(meta.pubkey, accounts)?);
        }
        
        // CPI to plugin program
        let cpi_ix = Instruction {
            program_id: &plugin.program_id,
            accounts: &cpi_accounts,
            data: &cpi_data,
        };
        
        // Invoke plugin validation (no signer seeds needed for plugin config)
        crate::util::invoke::invoke_signed_dynamic(
            &cpi_ix,
            &cpi_account_infos,
            &[], // No signer seeds needed
        )?;
    }
    // If no role/permission plugin exists, skip validation (optional in Pure External)
    
    // Get current account size and calculate new size
    let current_size = wallet_account_data.len();
    let num_authorities = wallet_account.num_authorities(wallet_account_data)?;
    
    // Calculate new authority size
    // Position (16 bytes) + authority_data + plugin_refs
    let plugin_refs_size = num_plugin_refs as usize * PluginRef::LEN;
    let new_authority_size = Position::LEN + new_authority_data_len as usize + plugin_refs_size;
    
    // Calculate new account size
    let authorities_offset = wallet_account.authorities_offset();
    let new_account_size = current_size + new_authority_size;
    
    // Reallocate account
    let new_account_size_aligned = core::alloc::Layout::from_size_align(
        new_account_size,
        8,
    )
    .map_err(|_| LazorkitError::InvalidAlignment)?
    .pad_to_align()
    .size();
    
    // Resize account (Pinocchio uses resize instead of realloc)
    wallet_account_info.resize(new_account_size_aligned)?;
    
    // Get mutable access after realloc
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };
    
    // Calculate new authority ID (increment from last authority or start at 0)
    let new_authority_id = if num_authorities == 0 {
        0
    } else {
        // Find last authority to get its ID
        let mut offset = authorities_offset;
        let mut last_id = 0u32;
        for _ in 0..num_authorities {
            if offset + Position::LEN > current_size {
                break;
            }
            // Parse Position manually to avoid alignment issues
            let position_id = u32::from_le_bytes([
                wallet_account_data[offset + 8],
                wallet_account_data[offset + 9],
                wallet_account_data[offset + 10],
                wallet_account_data[offset + 11],
            ]);
            let position_boundary = u32::from_le_bytes([
                wallet_account_data[offset + 12],
                wallet_account_data[offset + 13],
                wallet_account_data[offset + 14],
                wallet_account_data[offset + 15],
            ]);
            last_id = position_id;
            offset = position_boundary as usize;
        }
        last_id.wrapping_add(1)
    };
    
    // Calculate boundary (end of this authority)
    let new_authority_offset = if num_authorities == 0 {
        authorities_offset
    } else {
        // Find end of last authority
        let mut offset = authorities_offset;
        for _ in 0..num_authorities {
            if offset + Position::LEN > current_size {
                break;
            }
            // Parse Position boundary manually to avoid alignment issues
            let position_boundary = u32::from_le_bytes([
                wallet_account_data[offset + 12],
                wallet_account_data[offset + 13],
                wallet_account_data[offset + 14],
                wallet_account_data[offset + 15],
            ]);
            offset = position_boundary as usize;
        }
        offset
    };
    
    let new_boundary = new_authority_offset + new_authority_size;
    
    // Create Position structure
    let position = Position::new(
        new_authority_type,
        new_authority_data_len,
        num_plugin_refs,
        new_authority_id,
        new_boundary as u32,
    );
    
    // Write Position manually to avoid alignment issues
    // Position layout: authority_type (2) + authority_length (2) + num_plugin_refs (2) + padding (2) + id (4) + boundary (4)
    let mut position_bytes = [0u8; Position::LEN];
    position_bytes[0..2].copy_from_slice(&position.authority_type.to_le_bytes());
    position_bytes[2..4].copy_from_slice(&position.authority_length.to_le_bytes());
    position_bytes[4..6].copy_from_slice(&position.num_plugin_refs.to_le_bytes());
    // padding at 6..8 is already 0
    position_bytes[8..12].copy_from_slice(&position.id.to_le_bytes());
    position_bytes[12..16].copy_from_slice(&position.boundary.to_le_bytes());
    wallet_account_mut_data[new_authority_offset..new_authority_offset + Position::LEN]
        .copy_from_slice(&position_bytes);
    
    // Write authority data
    let auth_data_offset = new_authority_offset + Position::LEN;
    wallet_account_mut_data[auth_data_offset..auth_data_offset + authority_data.len()]
        .copy_from_slice(authority_data);
    
    // Write plugin refs (empty initially, but space is allocated)
    let plugin_refs_offset = auth_data_offset + authority_data.len();
    // Plugin refs are zero-initialized (already done by realloc)
    
    // Update num_authorities
    let new_num_authorities = num_authorities.wrapping_add(1);
    wallet_account.set_num_authorities(wallet_account_mut_data, new_num_authorities)?;
    
    // Ensure rent exemption
    let current_lamports = wallet_account_info.lamports();
    let required_lamports = Rent::get()?.minimum_balance(new_account_size_aligned);
    let lamports_needed = required_lamports.saturating_sub(current_lamports);
    
    if lamports_needed > 0 {
        Transfer {
            from: payer,
            to: wallet_account_info,
            lamports: lamports_needed,
        }
        .invoke()?;
    }
    
    Ok(())
}
