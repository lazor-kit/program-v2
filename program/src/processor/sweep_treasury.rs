use assertions::sol_assert_bytes_eq;
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    ProgramResult,
};

use crate::{error::AuthError, state::config::ConfigAccount};

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
    let admin_info = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let config_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let treasury_shard_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let destination_wallet = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !admin_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify Config PDA
    let (config_key, _config_bump) = find_program_address(&[b"config"], program_id);
    if !sol_assert_bytes_eq(config_pda.key().as_ref(), config_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Read config to verify admin
    let config_data = unsafe { config_pda.borrow_data_unchecked() };
    if config_data.len() < std::mem::size_of::<ConfigAccount>() {
        return Err(ProgramError::UninitializedAccount);
    }

    let config_account =
        unsafe { std::ptr::read_unaligned(config_data.as_ptr() as *const ConfigAccount) };

    if config_account.admin != *admin_info.key() {
        return Err(AuthError::PermissionDenied.into()); // Only admin can sweep
    }

    if shard_id >= config_account.num_shards {
        return Err(ProgramError::InvalidArgument); // Trying to sweep non-existent shard range
    }

    // Verify Treasury Shard PDA
    let shard_id_bytes = [shard_id];
    let (shard_key, _shard_bump) =
        find_program_address(&[b"treasury", &shard_id_bytes], program_id);
    if !sol_assert_bytes_eq(treasury_shard_pda.key().as_ref(), shard_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Transfer all lamports from treasury shard to destination wallet
    let shard_lamports = treasury_shard_pda.lamports();
    let dest_lamports = destination_wallet.lamports();

    unsafe {
        *destination_wallet.borrow_mut_lamports_unchecked() = dest_lamports
            .checked_add(shard_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *treasury_shard_pda.borrow_mut_lamports_unchecked() = 0;
    }

    // Erase any data if there was any (zero data)
    let shard_data = unsafe { treasury_shard_pda.borrow_mut_data_unchecked() };
    if !shard_data.is_empty() {
        shard_data.fill(0);
    }

    Ok(())
}
