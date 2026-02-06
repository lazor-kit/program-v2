use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;

/// Trait for defining the authentication logic for different authority types.
pub trait Authenticator {
    /// Authenticate the execution request.
    ///
    /// # Arguments
    /// * `accounts` - The full slice of accounts passed to the instruction.
    /// * `authority_data` - The mutable data of the authority account.
    /// * `auth_payload` - The specific authentication payload (e.g. signature, proof).
    /// * `signed_payload` - The message/payload that was signed (e.g. instruction args).
    fn authenticate(
        &self,
        accounts: &[AccountInfo],
        authority_data: &mut [u8],
        auth_payload: &[u8],
        signed_payload: &[u8],
        discriminator: &[u8],
    ) -> Result<(), ProgramError>;
}
