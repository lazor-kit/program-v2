use crate::state::authority::AuthorityAccountHeader;
use assertions::sol_assert_bytes_eq;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

/// Authenticates an Ed25519 authority.
///
/// Checks if the authority's pubkey matches a signer in the transaction.
/// Expects the account data buffer, which contains [Header] + [Pubkey].
///
/// # Arguments
/// * `auth_data` - The raw data of the authority account.
/// * `account_infos` - List of accounts involved in the transaction.
pub fn authenticate(auth_data: &[u8], account_infos: &[AccountInfo]) -> Result<(), ProgramError> {
    if auth_data.len() < std::mem::size_of::<AuthorityAccountHeader>() + 32 {
        return Err(ProgramError::InvalidAccountData);
    }

    // Header is at specific offset, but we just need variable data here for key
    let header_size = std::mem::size_of::<AuthorityAccountHeader>();
    // Ed25519 key is immediately after header
    let pubkey_bytes = &auth_data[header_size..header_size + 32];

    for account in account_infos {
        if account.is_signer() {
            if sol_assert_bytes_eq(account.key().as_ref(), pubkey_bytes, 32) {
                return Ok(());
            }
        }
    }

    Err(ProgramError::MissingRequiredSignature)
}
