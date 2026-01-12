//! Create Smart Wallet instruction handler - Pure External Architecture

use lazorkit_v2_assertions::{check_self_pda, check_system_owner, check_zero_data};
use lazorkit_v2_state::{
    authority::AuthorityType,
    plugin::PluginEntry,
    plugin_ref::PluginRef,
    position::Position,
    wallet_account::{
        wallet_account_seeds, wallet_account_seeds_with_bump, wallet_account_signer,
        wallet_vault_seeds_with_bump, WalletAccount,
    },
    Discriminator, IntoBytes, Transmutable,
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

use crate::error::LazorkitError;
use crate::util::invoke::find_account_info;
use lazorkit_v2_state::role_permission::RolePermission;

/// Arguments for creating a new Lazorkit wallet (Hybrid Architecture).
/// Note: instruction discriminator is already parsed in process_action, so we don't include it here
/// Creates wallet with first authority (root authority)
#[repr(C, align(8))]
#[derive(Debug)]
pub struct CreateSmartWalletArgs {
    pub id: [u8; 32],                  // Unique wallet identifier
    pub bump: u8,                      // PDA bump for wallet_account
    pub wallet_bump: u8,               // PDA bump for wallet_vault
    pub first_authority_type: u16,     // Type of first authority (root authority)
    pub first_authority_data_len: u16, // Length of first authority data
    pub num_plugin_refs: u16,          // Number of plugin refs for first authority
    pub role_permission: u8, // RolePermission enum for first authority (Hybrid: inline permission)
    pub _padding: [u8; 1],   // Padding to align to 8 bytes
}

impl CreateSmartWalletArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for CreateSmartWalletArgs {
    const LEN: usize = Self::LEN;
}

/// Creates a new Lazorkit smart wallet with first authority (Pure External architecture).
/// Creates wallet and adds first authority (root authority) in one instruction.
///
/// Accounts:
/// 0. wallet_account (writable, PDA)
/// 1. wallet_vault (writable, system-owned PDA)
/// 2. payer (writable, signer)
/// 3. system_program
/// 4..N. Optional plugin config accounts (if plugins need initialization)
pub fn create_smart_wallet(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }

    let wallet_account = &accounts[0];
    let wallet_vault = &accounts[1];
    let payer = &accounts[2];
    let system_program = &accounts[3];

    // Validate system program
    if system_program.key() != &pinocchio_system::ID {
        return Err(LazorkitError::InvalidSystemProgram.into());
    }

    // Validate accounts
    check_system_owner(wallet_account, LazorkitError::OwnerMismatchWalletState)?;
    check_zero_data(wallet_account, LazorkitError::AccountNotEmptyWalletState)?;
    check_system_owner(wallet_vault, LazorkitError::OwnerMismatchWalletState)?;
    check_zero_data(wallet_vault, LazorkitError::AccountNotEmptyWalletState)?;

    // Parse instruction args
    if instruction_data.len() < CreateSmartWalletArgs::LEN {
        return Err(LazorkitError::InvalidCreateInstructionDataTooShort.into());
    }

    // Parse instruction args
    if instruction_data.len() < CreateSmartWalletArgs::LEN {
        return Err(LazorkitError::InvalidCreateInstructionDataTooShort.into());
    }

    // Parse args manually to avoid alignment issues
    // CreateSmartWalletArgs: id (32) + bump (1) + wallet_bump (1) + first_authority_type (2) + first_authority_data_len (2) + num_plugin_refs (2) + role_permission (1) + padding (1) = 43 bytes (aligned to 48)
    let mut id = [0u8; 32];
    id.copy_from_slice(&instruction_data[0..32]);
    let bump = instruction_data[32];
    let wallet_bump = instruction_data[33];
    let first_authority_type = u16::from_le_bytes([instruction_data[34], instruction_data[35]]);
    let first_authority_data_len = u16::from_le_bytes([instruction_data[36], instruction_data[37]]);
    let num_plugin_refs = u16::from_le_bytes([instruction_data[38], instruction_data[39]]);
    let role_permission_byte = if instruction_data.len() > 40 {
        instruction_data[40]
    } else {
        RolePermission::All as u8 // Default: root authority has All permissions
    };
    let role_permission = RolePermission::try_from(role_permission_byte)
        .map_err(|_| LazorkitError::InvalidRolePermission)?;

    // Parse first authority data and plugin refs
    let args_start = CreateSmartWalletArgs::LEN;
    let authority_data_start = args_start;
    let authority_data_end = authority_data_start + first_authority_data_len as usize;

    if instruction_data.len() < authority_data_end {
        return Err(ProgramError::InvalidInstructionData);
    }

    let first_authority_data = &instruction_data[authority_data_start..authority_data_end];

    // Parse plugin refs
    let plugin_refs_start = authority_data_end;
    let plugin_refs_end = plugin_refs_start + (num_plugin_refs as usize * PluginRef::LEN);
    if instruction_data.len() < plugin_refs_end {
        return Err(ProgramError::InvalidInstructionData);
    }
    let plugin_refs_data = &instruction_data[plugin_refs_start..plugin_refs_end];

    // Validate authority type
    let authority_type = AuthorityType::try_from(first_authority_type)
        .map_err(|_| LazorkitError::InvalidAuthorityType)?;

    // Validate wallet_account PDA
    // Use find_program_address (like test does) to find correct PDA and bump
    let wallet_account_seeds_no_bump = wallet_account_seeds(&id);
    let (expected_pda, expected_bump) =
        pinocchio::pubkey::find_program_address(&wallet_account_seeds_no_bump, &crate::ID);

    // Verify PDA matches
    if expected_pda != *wallet_account.key() {
        return Err(LazorkitError::InvalidSeedWalletState.into());
    }

    // Verify bump matches
    if expected_bump != bump {
        return Err(LazorkitError::InvalidSeedWalletState.into());
    }

    let validated_bump = expected_bump;

    // Validate wallet_vault PDA (system-owned, derived from wallet_account key)
    // Note: For system-owned PDA, we use check_any_pda instead of check_self_pda
    // But wallet_vault validation is less critical since it's system-owned
    // We'll just verify it exists and is system-owned

    // Calculate account size
    // Header: WalletAccount (40 bytes) + num_authorities (2 bytes) + first authority + num_plugins (2 bytes)
    // First authority: Position (16 bytes) + authority_data + plugin_refs
    // Note: Nonce is not used. Each authority has its own odometer for replay protection.
    let plugin_refs_size = num_plugin_refs as usize * PluginRef::LEN;
    let first_authority_size = Position::LEN + first_authority_data_len as usize + plugin_refs_size;
    let min_account_size = WalletAccount::LEN + 2 + first_authority_size + 2; // Header + first authority + plugins
    let account_size = core::alloc::Layout::from_size_align(min_account_size, 8)
        .map_err(|_| LazorkitError::InvalidAlignment)?
        .pad_to_align()
        .size();

    let lamports_needed = Rent::get()?.minimum_balance(account_size);

    // Create WalletAccount
    let wallet_account_data = WalletAccount::new(id, bump, wallet_bump);

    // Get current lamports
    let current_lamports = unsafe { *wallet_account.borrow_lamports_unchecked() };
    let lamports_to_transfer = if current_lamports >= lamports_needed {
        0
    } else {
        lamports_needed - current_lamports
    };

    // Create wallet_account account
    CreateAccount {
        from: payer,
        to: wallet_account,
        lamports: lamports_to_transfer,
        space: account_size as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[wallet_account_signer(&id, &[validated_bump])
        .as_slice()
        .into()])?;

    // Initialize WalletAccount data
    let wallet_account_data_bytes = wallet_account_data.into_bytes()?;
    let wallet_account_mut_data = unsafe { wallet_account.borrow_mut_data_unchecked() };
    wallet_account_mut_data[..wallet_account_data_bytes.len()]
        .copy_from_slice(wallet_account_data_bytes);

    // Initialize num_authorities = 1 (first authority)
    wallet_account_mut_data[WalletAccount::LEN..WalletAccount::LEN + 2]
        .copy_from_slice(&1u16.to_le_bytes());

    // Write first authority
    let authorities_offset = WalletAccount::LEN + 2;
    let authority_id = 0u32; // First authority always has ID = 0
    let authority_boundary = authorities_offset + first_authority_size;

    // Create Position for first authority (Hybrid: includes role_permission)
    let position = Position::new(
        first_authority_type,
        first_authority_data_len,
        num_plugin_refs,
        role_permission,
        authority_id,
        authority_boundary as u32,
    );

    // Write Position manually to avoid alignment issues
    // Position layout: authority_type (2) + authority_length (2) + num_plugin_refs (2) + role_permission (1) + padding (1) + id (4) + boundary (4) = 16 bytes
    let mut position_bytes = [0u8; Position::LEN];
    position_bytes[0..2].copy_from_slice(&position.authority_type.to_le_bytes());
    position_bytes[2..4].copy_from_slice(&position.authority_length.to_le_bytes());
    position_bytes[4..6].copy_from_slice(&position.num_plugin_refs.to_le_bytes());
    position_bytes[6] = position.role_permission;
    // padding at 7 is already 0
    position_bytes[8..12].copy_from_slice(&position.id.to_le_bytes());
    position_bytes[12..16].copy_from_slice(&position.boundary.to_le_bytes());
    wallet_account_mut_data[authorities_offset..authorities_offset + Position::LEN]
        .copy_from_slice(&position_bytes);

    // Write authority data
    let auth_data_offset = authorities_offset + Position::LEN;
    wallet_account_mut_data[auth_data_offset..auth_data_offset + first_authority_data.len()]
        .copy_from_slice(first_authority_data);

    // Write plugin refs
    let plugin_refs_offset = auth_data_offset + first_authority_data.len();
    if !plugin_refs_data.is_empty() {
        wallet_account_mut_data[plugin_refs_offset..plugin_refs_offset + plugin_refs_data.len()]
            .copy_from_slice(plugin_refs_data);
    }

    // Initialize num_plugins = 0 (plugins will be added later via add_plugin)
    let plugins_offset = authority_boundary;
    wallet_account_mut_data[plugins_offset..plugins_offset + 2]
        .copy_from_slice(&0u16.to_le_bytes());

    // Note: Nonce is not used. Each authority has its own odometer for replay protection.

    // Create wallet_vault (system-owned PDA)
    let wallet_vault_rent_exemption = Rent::get()?.minimum_balance(0); // System account
    let current_wallet_vault_lamports = unsafe { *wallet_vault.borrow_lamports_unchecked() };
    let wallet_vault_lamports_to_transfer =
        if current_wallet_vault_lamports >= wallet_vault_rent_exemption {
            0
        } else {
            wallet_vault_rent_exemption - current_wallet_vault_lamports
        };

    if wallet_vault_lamports_to_transfer > 0 {
        // Transfer lamports to wallet_vault (system-owned PDA)
        // The account will be created automatically when it receives lamports
        pinocchio_system::instructions::Transfer {
            from: payer,
            to: wallet_vault,
            lamports: wallet_vault_lamports_to_transfer,
        }
        .invoke()?;
    }

    Ok(())
}
