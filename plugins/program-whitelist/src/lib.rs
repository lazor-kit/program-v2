//! Program Whitelist Plugin for Lazorkit V2
//!
//! This plugin allows interactions only with whitelisted programs.
//! It checks if the instruction's program_id is in the whitelist.

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};
use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{Transmutable, TransmutableMut};
use lazorkit_v2_instructions::InstructionIterator;

/// Plugin instruction discriminator
#[repr(u8)]
pub enum PluginInstruction {
    CheckPermission = 0,
    UpdateState = 1,
    UpdateConfig = 2,
    Initialize = 3,
}

/// Plugin config account structure
#[repr(C, align(8))]
#[derive(Debug)]
pub struct ProgramWhitelistConfig {
    pub discriminator: u8,
    pub bump: u8,
    pub wallet_account: Pubkey,  // WalletAccount this config belongs to (updated for Pure External)
    pub num_programs: u16,        // Number of whitelisted programs
    pub _padding: [u8; 4],
    // Followed by: program_ids (num_programs * 32 bytes)
}

impl ProgramWhitelistConfig {
    pub const LEN: usize = core::mem::size_of::<Self>();
    pub const SEED: &'static [u8] = b"program_whitelist_config";
    
    pub fn get_programs(&self, data: &[u8]) -> Result<&[[u8; 32]], ProgramError> {
        if data.len() < Self::LEN + (self.num_programs as usize * 32) {
            return Err(ProgramError::InvalidAccountData);
        }
        let programs_data = &data[Self::LEN..Self::LEN + (self.num_programs as usize * 32)];
        Ok(unsafe {
            core::slice::from_raw_parts(
                programs_data.as_ptr() as *const [u8; 32],
                self.num_programs as usize
            )
        })
    }
}

impl Transmutable for ProgramWhitelistConfig {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for ProgramWhitelistConfig {}

/// CheckPermission instruction arguments
#[repr(C, align(8))]
pub struct CheckPermissionArgs {
    pub instruction_data_len: u16,
    // Followed by: instruction_data (raw instruction bytes)
}

impl CheckPermissionArgs {
    pub const LEN: usize = 2;
}

/// UpdateConfig instruction arguments
#[repr(C, align(8))]
pub struct UpdateConfigArgs {
    pub instruction: u8,  // PluginInstruction::UpdateConfig
    pub num_programs: u16,
    // Followed by: program_ids (num_programs * 32 bytes)
}

/// Parse instruction discriminator
fn parse_instruction(data: &[u8]) -> Result<PluginInstruction, ProgramError> {
    if data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    match data[0] {
        0 => Ok(PluginInstruction::CheckPermission),
        1 => Ok(PluginInstruction::UpdateState),
        2 => Ok(PluginInstruction::UpdateConfig),
        3 => Ok(PluginInstruction::Initialize),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// Check if a program is whitelisted
fn is_program_whitelisted(
    config: &ProgramWhitelistConfig,
    config_data: &[u8],
    program_id: &Pubkey,
) -> Result<bool, ProgramError> {
    let whitelisted_programs = config.get_programs(config_data)?;
    
    for whitelisted in whitelisted_programs {
        if whitelisted == program_id.as_ref() {
            return Ok(true);
        }
    }
    
    Ok(false)
}

/// Handle CheckPermission instruction
/// 
/// CPI Call Format (Pure External):
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
    let wallet_vault = &accounts[2];
    
    // Validate config account
    check_self_owned(config_account, ProgramError::InvalidAccountData)?;
    let config_data = unsafe { config_account.borrow_data_unchecked() };
    
    if config_data.len() < ProgramWhitelistConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load config
    let config = unsafe {
        ProgramWhitelistConfig::load_unchecked(config_data)?
    };
    
    // Validate wallet_account matches
    if config.wallet_account != *wallet_account.key() {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Parse instruction data (Pure External format)
    if instruction_data.len() < 9 {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let authority_id = u32::from_le_bytes([
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
    
    // Parse instructions from payload
    let wallet_pubkey = wallet_account.key();
    let rkeys: &[&Pubkey] = &[];
    
    let ix_iter = InstructionIterator::new(
        accounts,
        instruction_payload,
        wallet_pubkey,
        rkeys,
    )?;
    
    // Check each instruction's program_id
    for ix_result in ix_iter {
        let instruction = ix_result?;
        
        // Check if program is whitelisted
        if !is_program_whitelisted(&config, config_data, instruction.program_id)? {
            return Err(ProgramError::Custom(1)); // Program not whitelisted
        }
    }
    
    Ok(())
}

/// Handle UpdateConfig instruction
fn handle_update_config(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 1 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let config_account = &accounts[0];
    
    // Validate config account
    check_self_owned(config_account, ProgramError::InvalidAccountOwner)?;
    let config_data = unsafe { config_account.borrow_mut_data_unchecked() };
    
    if config_data.len() < ProgramWhitelistConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Parse UpdateConfig args
    if instruction_data.len() < 3 {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let num_programs = u16::from_le_bytes([
        instruction_data[1],
        instruction_data[2],
    ]);
    
    let programs_data_len = num_programs as usize * 32;
    if instruction_data.len() < 3 + programs_data_len {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    // Update config
    let mut config = unsafe {
        ProgramWhitelistConfig::load_mut_unchecked(config_data)?
    };
    
    config.num_programs = num_programs;
    
    // Copy program IDs
    let programs_data = &instruction_data[3..3 + programs_data_len];
    config_data[ProgramWhitelistConfig::LEN..ProgramWhitelistConfig::LEN + programs_data_len]
        .copy_from_slice(programs_data);
    
    Ok(())
}

/// Handle UpdateState instruction (called after execution)
fn handle_update_state(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // For ProgramWhitelist plugin, no state update needed
    // This is a no-op
    Ok(())
}

/// Handle Initialize instruction
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
    let system_program = &accounts[3];
    
    // Parse num_programs and program_ids from instruction data
    // Format: [instruction: u8, num_programs: u16, program_ids: num_programs * 32 bytes]
    if instruction_data.len() < 3 {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let num_programs = u16::from_le_bytes([
        instruction_data[1],
        instruction_data[2],
    ]);
    
    let programs_data_len = num_programs as usize * 32;
    if instruction_data.len() < 3 + programs_data_len {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    // Initialize config account
    use pinocchio_system::instructions::Transfer;
    use pinocchio::sysvars::{rent::Rent, Sysvar};
    use lazorkit_v2_state::Discriminator;
    
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(ProgramWhitelistConfig::LEN + programs_data_len);
    
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
    let config_data = unsafe { config_account.borrow_mut_data_unchecked() };
    if config_data.len() < ProgramWhitelistConfig::LEN + programs_data_len {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Write header
    config_data[0] = Discriminator::WalletAccount as u8;
    config_data[1] = 0; // bump
    config_data[2..34].copy_from_slice(wallet_account.key().as_ref());
    config_data[34..36].copy_from_slice(&num_programs.to_le_bytes());
    // padding at 36..40
    
    // Write program IDs
    let programs_data = &instruction_data[3..3 + programs_data_len];
    config_data[ProgramWhitelistConfig::LEN..ProgramWhitelistConfig::LEN + programs_data_len]
        .copy_from_slice(programs_data);
    
    // Set owner
    unsafe {
        config_account.assign(program_id);
    }
    
    Ok(())
}

/// Program entrypoint
#[cfg(not(feature = "no-entrypoint"))]
pinocchio::entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Parse instruction
    let instruction = parse_instruction(instruction_data)?;
    
    match instruction {
        PluginInstruction::CheckPermission => {
            handle_check_permission(accounts, instruction_data)
        },
        PluginInstruction::UpdateState => {
            handle_update_state(accounts, instruction_data)
        },
        PluginInstruction::UpdateConfig => {
            handle_update_config(accounts, instruction_data)
        },
        PluginInstruction::Initialize => {
            handle_initialize(program_id, accounts, instruction_data)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
