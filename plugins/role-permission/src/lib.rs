//! Role/Permission Plugin for Lazorkit V2
//!
//! This plugin implements basic role/permission checking similar to Swig's
//! All and ManageAuthority actions. It allows or denies operations based on
//! configured permissions.

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};
use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{Transmutable, TransmutableMut, Discriminator};

pinocchio::default_allocator!();
pinocchio::default_panic_handler!();

/// Plugin instruction discriminator
#[repr(u8)]
pub enum PluginInstruction {
    CheckPermission = 0,
    UpdateState = 1,
    ValidateAddAuthority = 2,
    Initialize = 3,
}

/// Permission types (similar to Swig)
#[repr(u8)]
pub enum PermissionType {
    All = 0,                    // Allow all operations
    ManageAuthority = 1,        // Allow authority management only
    AllButManageAuthority = 2,  // Allow all except authority management
}

/// Plugin config account structure
#[repr(C, align(8))]
#[derive(Debug)]
pub struct RolePermissionConfig {
    pub discriminator: u8,
    pub bump: u8,
    pub wallet_account: Pubkey,  // WalletAccount this config belongs to
    pub permission_type: u8,     // PermissionType
    pub _padding: [u8; 6],
}

impl RolePermissionConfig {
    pub const LEN: usize = core::mem::size_of::<Self>();
    pub const SEED: &'static [u8] = b"role_permission_config";
    
    pub fn new(wallet_account: Pubkey, bump: u8, permission_type: PermissionType) -> Self {
        Self {
            discriminator: Discriminator::WalletAccount as u8,
            bump,
            wallet_account,
            permission_type: permission_type as u8,
            _padding: [0; 6],
        }
    }
}

impl Transmutable for RolePermissionConfig {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for RolePermissionConfig {}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let instruction = instruction_data[0];
    
    match instruction {
        0 => handle_check_permission(accounts, instruction_data),
        1 => handle_update_state(accounts, instruction_data),
        2 => handle_validate_add_authority(accounts, instruction_data),
        3 => handle_initialize(program_id, accounts, instruction_data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// CheckPermission instruction handler
/// 
/// CPI Call Format:
/// [0] - PluginInstruction::CheckPermission (u8)
/// [1-4] - authority_id (u32, little-endian)
/// [5-8] - authority_data_len (u32, little-endian)
/// [9..9+authority_data_len] - authority_data
/// [9+authority_data_len..9+authority_data_len+4] - instruction_data_len (u32, little-endian)
/// [9+authority_data_len+4..] - instruction_data
fn handle_check_permission(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 3 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let config_account = &accounts[0];
    let wallet_account = &accounts[1];
    let _wallet_vault = &accounts[2];
    
    // Validate config account
    check_self_owned(config_account, ProgramError::InvalidAccountData)?;
    
    // Load plugin config
    let config_data = unsafe { config_account.borrow_data_unchecked() };
    if config_data.len() < RolePermissionConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let config = unsafe { RolePermissionConfig::load_unchecked(config_data)? };
    
    // Validate wallet_account matches
    if config.wallet_account != *wallet_account.key() {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Parse instruction data
    if instruction_data.len() < 9 {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let _authority_id = u32::from_le_bytes([
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
        instruction_data[4],
    ]);
    
    let authority_data_len = u32::from_le_bytes([
        instruction_data[5],
        instruction_data[6],
        instruction_data[7],
        instruction_data[8],
    ]) as usize;
    
    if instruction_data.len() < 9 + authority_data_len + 4 {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let instruction_payload_len = u32::from_le_bytes([
        instruction_data[9 + authority_data_len],
        instruction_data[9 + authority_data_len + 1],
        instruction_data[9 + authority_data_len + 2],
        instruction_data[9 + authority_data_len + 3],
    ]) as usize;
    
    if instruction_data.len() < 9 + authority_data_len + 4 + instruction_payload_len {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let instruction_payload = &instruction_data[9 + authority_data_len + 4..9 + authority_data_len + 4 + instruction_payload_len];
    
    // Check permission based on type
    let permission_type = match config.permission_type {
        0 => PermissionType::All,
        1 => PermissionType::ManageAuthority,
        2 => PermissionType::AllButManageAuthority,
        _ => return Err(ProgramError::InvalidAccountData),
    };
    
    // Parse instruction to check if it's authority management
    let is_authority_management = if instruction_payload.len() >= 2 {
        // Check if instruction is AddAuthority, RemoveAuthority, UpdateAuthority, CreateSession
        let instruction_discriminator = u16::from_le_bytes([
            instruction_payload[0],
            instruction_payload[1],
        ]);
        // Lazorkit instruction discriminators:
        // AddAuthority = 2, RemoveAuthority = 7, UpdateAuthority = 6, CreateSession = 8
        matches!(instruction_discriminator, 2 | 6 | 7 | 8)
    } else {
        false
    };
    
    match permission_type {
        PermissionType::All => {
            // Allow all operations
            Ok(())
        },
        PermissionType::ManageAuthority => {
            // Only allow authority management operations
            if is_authority_management {
                Ok(())
            } else {
                Err(ProgramError::Custom(1)) // Permission denied
            }
        },
        PermissionType::AllButManageAuthority => {
            // Allow all except authority management
            if is_authority_management {
                Err(ProgramError::Custom(1)) // Permission denied
            } else {
                Ok(())
            }
        },
    }
}

/// UpdateState instruction handler
/// Called after instruction execution to update plugin state
fn handle_update_state(
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 1 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let config_account = &accounts[0];
    
    // Validate config account
    check_self_owned(config_account, ProgramError::InvalidAccountData)?;
    
    // For RolePermission plugin, no state update needed
    // This is a no-op
    Ok(())
}

/// ValidateAddAuthority instruction handler
/// Called when adding a new authority to validate it
fn handle_validate_add_authority(
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let config_account = &accounts[0];
    let wallet_account = &accounts[1];
    
    // Validate config account
    check_self_owned(config_account, ProgramError::InvalidAccountData)?;
    
    // Load plugin config
    let config_data = unsafe { config_account.borrow_data_unchecked() };
    if config_data.len() < RolePermissionConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let config = unsafe { RolePermissionConfig::load_unchecked(config_data)? };
    
    // Validate wallet_account matches
    if config.wallet_account != *wallet_account.key() {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // For RolePermission plugin, we allow adding any authority
    // In a more sophisticated implementation, we could check authority type, etc.
    // No validation needed for now
    Ok(())
}

/// Initialize instruction handler
/// Creates and initializes the plugin config account
fn handle_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let config_account = &accounts[0];
    let wallet_account = &accounts[1];
    let payer = &accounts[2];
    let _system_program = &accounts[3];
    
    // Parse permission type from instruction data
    if instruction_data.len() < 2 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let permission_type = instruction_data[1];
    if permission_type > 2 {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    // Initialize config account
    use pinocchio_system::instructions::Transfer;
    use pinocchio::sysvars::{rent::Rent, Sysvar};
    
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(RolePermissionConfig::LEN);
    
    if config_account.lamports() < required_lamports {
        let lamports_needed = required_lamports - config_account.lamports();
        Transfer {
            from: payer,
            to: config_account,
            lamports: lamports_needed,
        }
        .invoke()?;
    }
    
    // Write config
    let config = RolePermissionConfig::new(
        *wallet_account.key(),
        0, // bump will be set by PDA derivation
        match permission_type {
            0 => PermissionType::All,
            1 => PermissionType::ManageAuthority,
            2 => PermissionType::AllButManageAuthority,
            _ => return Err(ProgramError::InvalidInstructionData),
        },
    );
    
    let config_data = unsafe { config_account.borrow_mut_data_unchecked() };
    if config_data.len() < RolePermissionConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let config_bytes = unsafe {
        core::slice::from_raw_parts(&config as *const RolePermissionConfig as *const u8, RolePermissionConfig::LEN)
    };
    config_data[..RolePermissionConfig::LEN].copy_from_slice(config_bytes);
    
    // Set owner
    unsafe {
        config_account.assign(program_id);
    }
    
    Ok(())
}
