use pinocchio::{
    account_info::AccountInfo, entrypoint, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::processor::{
    close_session, close_wallet, create_session, create_wallet, execute, init_treasury_shard,
    initialize_config, manage_authority, sweep_treasury, transfer_ownership, update_config,
};

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
        5 => create_session::process(program_id, accounts, data),
        6 => initialize_config::process(program_id, accounts, data),
        7 => update_config::process(program_id, accounts, data),
        8 => close_session::process(program_id, accounts, data),
        9 => close_wallet::process(program_id, accounts, data),
        10 => sweep_treasury::process(program_id, accounts, data),
        11 => init_treasury_shard::process(program_id, accounts, data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
