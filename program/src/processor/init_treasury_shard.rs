use assertions::sol_assert_bytes_eq;
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::rent::Rent,
    ProgramResult,
};

use crate::state::{config::ConfigAccount, AccountDiscriminator};

/// Arguments:
/// - `shard_id`: u8
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let shard_id = instruction_data[0];

    let account_info_iter = &mut accounts.iter();
    let payer_info = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let config_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let treasury_shard_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let rent_sysvar = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !payer_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify Config PDA
    let (config_key, _config_bump) = find_program_address(&[b"config"], program_id);
    if !sol_assert_bytes_eq(config_pda.key().as_ref(), config_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Read config to verify shard_id is within bounds
    let config_data = unsafe { config_pda.borrow_data_unchecked() };
    if config_data.len() < std::mem::size_of::<ConfigAccount>() {
        return Err(ProgramError::UninitializedAccount);
    }

    let config_account =
        unsafe { std::ptr::read_unaligned(config_data.as_ptr() as *const ConfigAccount) };

    if config_account.discriminator != AccountDiscriminator::Config as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    if shard_id >= config_account.num_shards {
        return Err(ProgramError::InvalidArgument);
    }

    // Verify Treasury Shard PDA
    let shard_id_bytes = [shard_id];
    let (shard_key, shard_bump) = find_program_address(&[b"treasury", &shard_id_bytes], program_id);
    if !sol_assert_bytes_eq(treasury_shard_pda.key().as_ref(), shard_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    // A treasury shard account has NO structure/data. It's just a raw system account
    // owned by the system program (or our program) with a balance to be rent-exempt.
    // If it has 0 bytes data, minimum balance is 890,880 lamports.
    let rent = Rent::from_account_info(rent_sysvar)?;
    let rent_lamports = rent.minimum_balance(0);

    let shard_bump_arr = [shard_bump];
    let pda_seeds = [
        Seed::from(b"treasury"),
        Seed::from(&shard_id_bytes),
        Seed::from(&shard_bump_arr),
    ];

    // Initialize as a 0-byte PDA owned by system program (or program_id, but our init func assigns to program_id)
    // Actually, it doesn't matter much if it's owned by System Program or our Program,
    // as long as the PDA signs to transfer funds out later. Let's make it owned by program_id
    crate::utils::initialize_pda_account(
        payer_info,
        treasury_shard_pda,
        system_program,
        0, // 0 space
        rent_lamports,
        program_id,
        &pda_seeds,
    )?;

    Ok(())
}
