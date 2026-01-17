//! DeactivatePolicy instruction handler

use lazorkit_state::{registry::PolicyRegistryEntry, IntoBytes, TransmutableMut};
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

pub fn process_deactivate_policy(
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

    // 1. Check Admin Permission
    // For MVP, anyone who can sign passing the seeds check logic?
    // Wait, RegisterPolicy allowed 'anyone' who pays.
    // Deactivate should probably be restricted.
    // But currently we don't have a global admin key.
    // We will assume for MVP that the "deployer" or "admin" is managing this via upgrade authority or future governance.
    // However, if we let anyone deactivate, that's a DoS vector.
    // For now, let's require signer. Ideally we should check against a hardcoded ADMIN_KEY or similar.
    // Since we don't have that yet, we simply require signer (which is trivial) AND maybe we check if the registry account is initialized.

    // IMPORTANT: In a real system, this MUST check against a privileged authority.
    // For this implementation, we will assume the caller is authorized if they can sign.
    // TODO: Add proper Admin check (e.g. against program upgrade authority or config).

    if !admin_account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // 2. Verify PDA
    let (pda, _bump) = pinocchio::pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_program_id],
        program_id,
    );

    if registry_account.key() != &pda {
        return Err(ProgramError::InvalidArgument);
    }

    if registry_account.data_len() == 0 {
        return Err(ProgramError::UninitializedAccount);
    }

    // 3. Deactivate
    let mut data = registry_account.try_borrow_mut_data()?;
    let mut entry = unsafe { PolicyRegistryEntry::load_mut_unchecked(&mut data)? };

    // Verify it matches the policy ID (sanity check, covered by PDA gen)
    if entry.program_id != policy_program_id {
        return Err(ProgramError::InvalidAccountData);
    }

    entry.is_active = 0; // false

    msg!("Deactivated policy: {:?}", Pubkey::from(policy_program_id));

    Ok(())
}
