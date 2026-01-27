use crate::auth::traits::Authenticator;
use crate::state::authority::AuthorityAccountHeader;
use assertions::sol_assert_bytes_eq;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

pub struct Ed25519Authenticator;

impl Authenticator for Ed25519Authenticator {
    fn authenticate(
        &self,
        accounts: &[AccountInfo],
        authority_data: &mut [u8],
        _auth_payload: &[u8],
        _signed_payload: &[u8],
    ) -> Result<(), ProgramError> {
        if authority_data.len() < std::mem::size_of::<AuthorityAccountHeader>() + 32 {
            return Err(ProgramError::InvalidAccountData);
        }

        // Header is at specific offset, but we just need variable data here for key
        let header_size = std::mem::size_of::<AuthorityAccountHeader>();
        // Ed25519 key is immediately after header
        let pubkey_bytes = &authority_data[header_size..header_size + 32];

        for account in accounts {
            if account.is_signer() && sol_assert_bytes_eq(account.key().as_ref(), pubkey_bytes, 32)
            {
                return Ok(());
            }
        }

        Err(ProgramError::MissingRequiredSignature)
    }
}
