use pinocchio::{
    account_info::AccountInfo, entrypoint, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::processor::{create_wallet, execute, manage_authority, transfer_ownership};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let (discriminator, data) = instruction_data.split_first().unwrap();

    match discriminator {
        0 => create_wallet::process(program_id, accounts, data),
        1 => manage_authority::process_add_authority(program_id, accounts, data),
        2 => manage_authority::process_remove_authority(program_id, accounts, data),
        3 => transfer_ownership::process(program_id, accounts, data),
        4 => execute::process(program_id, accounts, data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
