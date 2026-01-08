//! All Permission Plugin for Lazorkit V2
//!
//! This is a simple plugin that allows all operations.
//! Useful for testing or for authorities that need unrestricted access.

use pinocchio::{
    account_info::AccountInfo,
    entrypoint,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};
use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{Transmutable, TransmutableMut};

/// Plugin instruction discriminator
#[repr(u8)]
pub enum PluginInstruction {
    CheckPermission = 0,
}

/// Plugin config account structure (minimal, just for identification)
#[repr(C, align(8))]
#[derive(Debug)]
pub struct AllPermissionConfig {
    pub discriminator: u8,
    pub bump: u8,
    pub wallet_state: Pubkey,  // WalletState account this config belongs to
    pub _padding: [u8; 6],
}

impl AllPermissionConfig {
    pub const LEN: usize = core::mem::size_of::<Self>();
    pub const SEED: &'static [u8] = b"all_permission_config";
}

impl Transmutable for AllPermissionConfig {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for AllPermissionConfig {}

/// CheckPermission instruction arguments
#[repr(C, align(8))]
pub struct CheckPermissionArgs {
    pub instruction_data_len: u16,
    // Followed by: instruction_data (raw instruction bytes)
}

impl CheckPermissionArgs {
    pub const LEN: usize = 2;
}

/// Parse instruction discriminator
fn parse_instruction(data: &[u8]) -> Result<PluginInstruction, ProgramError> {
    if data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    match data[0] {
        0 => Ok(PluginInstruction::CheckPermission),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// Handle CheckPermission instruction
/// This plugin always allows everything, so we just validate the config exists
fn handle_check_permission(
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let config_account = &accounts[0];
    
    // Validate config account exists and is owned by this program
    check_self_owned(config_account, ProgramError::InvalidAccountOwner)?;
    let config_data = unsafe { config_account.borrow_data_unchecked() };
    
    if config_data.len() < AllPermissionConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Config exists and is valid - allow everything
    Ok(())
}

/// Program entrypoint
#[entrypoint]
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Parse instruction
    let instruction = parse_instruction(instruction_data)?;
    
    match instruction {
        PluginInstruction::CheckPermission => {
            handle_check_permission(accounts, instruction_data)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
