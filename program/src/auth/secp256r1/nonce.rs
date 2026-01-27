use pinocchio::{
    account_info::{AccountInfo, Ref},
    program_error::ProgramError,
};

use crate::auth::secp256r1::slothashes::SlotHashes;
use crate::error::AuthError;

#[derive(Clone, Copy)]
pub struct TruncatedSlot(pub u16);

impl TruncatedSlot {
    pub fn new(untruncated_slot: u64) -> Self {
        Self((untruncated_slot % 1000) as u16)
    }

    pub fn get_index_difference(&self, other: &Self) -> u16 {
        if self.0 >= other.0 {
            self.0 - other.0
        } else {
            self.0 + (1000 - other.0)
        }
    }
}

use crate::utils::get_stack_height;

pub fn validate_nonce(
    slothashes_sysvar: &AccountInfo,
    submitted_slot: &TruncatedSlot,
) -> Result<[u8; 32], ProgramError> {
    // Ensure the program isn't being called via CPI
    if get_stack_height() > 1 {
        return Err(AuthError::PermissionDenied.into()); // Mapping CPINotAllowed error
    }

    let slothashes = SlotHashes::<Ref<[u8]>>::try_from(slothashes_sysvar)?;

    // Get current slothash (index 0)
    let most_recent_slot_hash = slothashes.get_slot_hash(0)?;
    let truncated_most_recent_slot = TruncatedSlot::new(most_recent_slot_hash.height);

    let index_difference = truncated_most_recent_slot.get_index_difference(submitted_slot);

    if index_difference >= 150 {
        return Err(AuthError::InvalidSignatureAge.into());
    }

    let slot_hash = slothashes.get_slot_hash(index_difference as usize)?;

    Ok(slot_hash.hash)
}
