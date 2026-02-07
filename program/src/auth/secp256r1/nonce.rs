use pinocchio::{
    account_info::{AccountInfo, Ref},
    program_error::ProgramError,
};

use crate::auth::secp256r1::slothashes::SlotHashes;
use crate::error::AuthError;

// TruncatedSlot removed (Issue #16)

use crate::utils::get_stack_height;

pub fn validate_nonce(
    slothashes_sysvar: &AccountInfo,
    submitted_slot: u64,
) -> Result<[u8; 32], ProgramError> {
    // Ensure the program isn't being called via CPI
    if get_stack_height() > 1 {
        return Err(AuthError::PermissionDenied.into()); // Mapping CPINotAllowed error
    }

    let slothashes = SlotHashes::<Ref<[u8]>>::try_from(slothashes_sysvar)?;

    // Get current slothash (index 0)
    let most_recent_slot_hash = slothashes.get_slot_hash(0)?;
    let current_slot = most_recent_slot_hash.height;

    // Check if submitted slot is in the future
    if submitted_slot > current_slot {
        return Err(AuthError::InvalidSignatureAge.into());
    }

    let index_difference = current_slot - submitted_slot;

    if index_difference >= 150 {
        return Err(AuthError::InvalidSignatureAge.into());
    }

    // Assuming SlotHashes stores hashes in descending order of slot height
    let slot_hash = slothashes.get_slot_hash(index_difference as usize)?;

    Ok(slot_hash.hash)
}
