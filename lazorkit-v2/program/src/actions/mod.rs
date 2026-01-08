//! Action handlers for Lazorkit V2 instructions.

use pinocchio::{account_info::AccountInfo, program_error::ProgramError, ProgramResult};
use lazorkit_v2_state::AccountClassification;
use crate::instruction::LazorkitInstruction;
use crate::error::LazorkitError;
use num_enum::FromPrimitive;

pub mod create_smart_wallet;
pub mod sign;
pub mod add_authority;
pub mod add_plugin;
pub mod remove_authority;
pub mod update_authority;
pub mod remove_plugin;
pub mod update_plugin;
pub mod create_session;

/// Dispatches to the appropriate action handler based on the instruction.
pub fn process_action(
    accounts: &[AccountInfo],
    account_classification: &mut [AccountClassification],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    // Parse instruction discriminator (first 2 bytes)
    if instruction_data.len() < 2 {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    let instruction_u16 = unsafe { *(instruction_data.get_unchecked(..2).as_ptr() as *const u16) };
    
    // Match directly with instruction_u16 to avoid from_primitive issues
    match instruction_u16 {
        0 => {
            create_smart_wallet::create_smart_wallet(accounts, &instruction_data[2..])
        },
        1 => {
            sign::sign(accounts, &instruction_data[2..], account_classification)
        },
        2 => {
            add_authority::add_authority(accounts, &instruction_data[2..])
        },
        3 => {
            add_plugin::add_plugin(accounts, &instruction_data[2..])
        },
        4 => {
            remove_plugin::remove_plugin(accounts, &instruction_data[2..])
        },
        5 => {
            update_plugin::update_plugin(accounts, &instruction_data[2..])
        },
        6 => {
            update_authority::update_authority(accounts, &instruction_data[2..])
        },
        7 => {
            remove_authority::remove_authority(accounts, &instruction_data[2..])
        },
        8 => {
            create_session::create_session(accounts, &instruction_data[2..])
        },
        _ => {
            // Use from_primitive for other instructions (should not happen for valid instructions)
            Err(ProgramError::InvalidInstructionData)
        },
    }
}
