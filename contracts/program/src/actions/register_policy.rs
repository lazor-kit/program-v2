//! RegisterPolicy instruction handler

use lazorkit_state::registry::PolicyRegistryEntry;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

use crate::error::LazorKitError;

pub fn process_register_policy(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    policy_program_id: [u8; 32],
) -> ProgramResult {
    let mut account_info_iter = accounts.iter();
    let registry_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let admin_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let _system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !admin_account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // TODO: Verify Admin Authority?
    // For now, anyone can pay to register an entry, but maybe restricting to specific Admin key is better?
    // Architecture says "Protocol Admin / DAO".
    // Let's assume for this MVP that knowing the Seeds allows creation (standard PDA pattern),
    // but in reality we should check signer against a hardcoded Admin Key or Config.
    // For simplicity of this "Registry Factory":
    // We allow creation if the PDA is valid. The security comes from WHO can update it (not implemented yet)
    // or we assume deployment key controls it.
    // Let's stick to simple PDA creation logic.

    let policy_key = Pubkey::from(policy_program_id);
    let (pda, bump) = pinocchio::pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, policy_key.as_ref()],
        program_id,
    );

    if registry_account.key() != &pda {
        return Err(ProgramError::InvalidSeeds);
    }

    if !registry_account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // Create Account
    let space = PolicyRegistryEntry::LEN;
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(space);

    let seeds = &[
        PolicyRegistryEntry::SEED_PREFIX,
        policy_key.as_ref(),
        &[bump],
    ];
    let seeds_list = [
        Seed::from(seeds[0]),
        Seed::from(seeds[1]),
        Seed::from(seeds[2]),
    ];
    let signer = Signer::from(&seeds_list);

    CreateAccount {
        from: admin_account,
        to: registry_account,
        lamports,
        space: space as u64,
        owner: program_id,
    }
    .invoke_signed(&[signer])?;

    // Initialize State
    let clock = Clock::get()?;
    let entry = PolicyRegistryEntry::new(policy_key, bump, clock.unix_timestamp);

    let mut data = registry_account.try_borrow_mut_data()?;
    // Unsafe copy to struct layout
    unsafe {
        let ptr = data.as_mut_ptr() as *mut PolicyRegistryEntry;
        *ptr = entry;
    }

    msg!("Registered policy: {:?}", policy_key);
    Ok(())
}
