use crate::{
    error::AuthError,
    state::{deferred::DeferredExecAccount, AccountDiscriminator},
};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

/// Process the ReclaimDeferred instruction.
///
/// Closes an expired DeferredExec account and refunds rent to the original payer.
/// Only the original payer can reclaim, and only after the authorization has expired.
///
/// # Accounts:
/// 1. `[signer]` Payer (must match stored payer)
/// 2. `[writable]` DeferredExec PDA (closed)
/// 3. `[writable]` Refund destination
///
/// # Instruction Data (after discriminator):
///   (none)
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let payer = accounts
        .first()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let deferred_pda = accounts
        .get(1)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let refund_dest = accounts
        .get(2)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // Validate signer
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify ownership
    if deferred_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Read DeferredExec account
    let deferred_data = unsafe { deferred_pda.borrow_mut_data_unchecked() };
    if deferred_data.len() < std::mem::size_of::<DeferredExecAccount>() {
        return Err(ProgramError::InvalidAccountData);
    }

    let deferred = unsafe {
        std::ptr::read_unaligned(deferred_data.as_ptr() as *const DeferredExecAccount)
    };

    if deferred.discriminator != AccountDiscriminator::DeferredExec as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    // Only the original payer can reclaim
    if deferred.payer != *payer.key() {
        return Err(AuthError::UnauthorizedReclaim.into());
    }

    // Can only reclaim after expiry
    let clock = Clock::get()?;
    if clock.slot <= deferred.expires_at {
        return Err(AuthError::DeferredAuthorizationNotExpired.into());
    }

    // Close the account — zero data and drain lamports
    for byte in deferred_data.iter_mut() {
        *byte = 0;
    }

    let deferred_lamports = deferred_pda.lamports();
    unsafe {
        *deferred_pda.borrow_mut_lamports_unchecked() = 0;
        *refund_dest.borrow_mut_lamports_unchecked() += deferred_lamports;
    }

    Ok(())
}
