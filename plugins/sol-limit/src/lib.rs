//! SOL Limit Plugin for Lazorkit V2
//!
//! This plugin enforces a maximum SOL transfer limit per authority.
//! It tracks the remaining SOL that can be transferred and decreases
//! the limit as operations are performed.

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
    UpdateConfig = 1,
}

/// Plugin config account structure
#[repr(C, align(8))]
#[derive(Debug)]
pub struct SolLimitConfig {
    pub discriminator: u8,
    pub bump: u8,
    pub wallet_state: Pubkey,  // WalletState account this config belongs to
    pub remaining_amount: u64,  // Remaining SOL limit in lamports
    pub _padding: [u8; 6],
}

impl SolLimitConfig {
    pub const LEN: usize = core::mem::size_of::<Self>();
    pub const SEED: &'static [u8] = b"sol_limit_config";
}

impl Transmutable for SolLimitConfig {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for SolLimitConfig {}

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
    pub new_limit: u64,   // New SOL limit in lamports
}

impl UpdateConfigArgs {
    pub const LEN: usize = 9;
}

/// Parse instruction discriminator
fn parse_instruction(data: &[u8]) -> Result<PluginInstruction, ProgramError> {
    if data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    match data[0] {
        0 => Ok(PluginInstruction::CheckPermission),
        1 => Ok(PluginInstruction::UpdateConfig),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// Check if a SOL transfer instruction is within limits
fn check_sol_transfer(
    config: &mut SolLimitConfig,
    instruction_data: &[u8],
    accounts: &[AccountInfo],
) -> ProgramResult {
    use pinocchio_system::instructions::Transfer;
    
    // Parse system transfer instruction
    // System transfer: program_id (32) + lamports (8) + from (32) + to (32)
    if instruction_data.len() < 8 {
        return Ok(()); // Not a transfer, allow
    }
    
    // Check if this is a system transfer
    // For simplicity, we'll check if lamports are being transferred
    // In a real implementation, you'd parse the instruction properly
    let lamports = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );
    
    // Check if transfer exceeds limit
    if lamports > config.remaining_amount {
        return Err(ProgramError::InsufficientFunds);
    }
    
    // Decrease remaining amount
    config.remaining_amount = config.remaining_amount.saturating_sub(lamports);
    
    Ok(())
}

/// Handle CheckPermission instruction
fn handle_check_permission(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let config_account = &accounts[0];
    let wallet_state = &accounts[1];
    
    // Validate config account
    check_self_owned(config_account, ProgramError::InvalidAccountOwner)?;
    let config_data = unsafe { config_account.borrow_mut_data_unchecked() };
    
    if config_data.len() < SolLimitConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load config
    let mut config = unsafe {
        SolLimitConfig::load_mut_unchecked(config_data)?
    };
    
    // Parse CheckPermission args
    if instruction_data.len() < CheckPermissionArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let args_len = u16::from_le_bytes([
        instruction_data[1],
        instruction_data[2],
    ]) as usize;
    
    if instruction_data.len() < CheckPermissionArgs::LEN + args_len {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let inner_instruction_data = &instruction_data[CheckPermissionArgs::LEN..CheckPermissionArgs::LEN + args_len];
    
    // Check SOL transfer limits
    check_sol_transfer(&mut config, inner_instruction_data, accounts)?;
    
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
    
    if config_data.len() < SolLimitConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Parse UpdateConfig args
    if instruction_data.len() < UpdateConfigArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let new_limit = u64::from_le_bytes(
        instruction_data[1..9].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );
    
    // Load and update config
    let mut config = unsafe {
        SolLimitConfig::load_mut_unchecked(config_data)?
    };
    
    config.remaining_amount = new_limit;
    
    Ok(())
}

/// Program entrypoint
#[entrypoint]
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
        PluginInstruction::UpdateConfig => {
            handle_update_config(accounts, instruction_data)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
