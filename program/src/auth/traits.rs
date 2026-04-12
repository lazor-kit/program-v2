use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::Pubkey;

/// Trait for defining the authentication logic for different authority types.
pub trait Authenticator {
    /// Authenticate the execution request.
    ///
    /// # Arguments
    /// * `accounts` - The full slice of accounts passed to the instruction.
    /// * `authority_data` - The mutable data of the authority account.
    /// * `auth_payload` - The specific authentication payload (e.g. signature, proof).
    /// * `signed_payload` - The message/payload that was signed (e.g. instruction args).
    /// * `discriminator` - The instruction opcode byte(s).
    /// * `program_id` - This program's public key (included in Secp256r1 challenge hash).
    fn authenticate(
        &self,
        accounts: &[AccountInfo],
        authority_data: &mut [u8],
        auth_payload: &[u8],
        signed_payload: &[u8],
        discriminator: &[u8],
        program_id: &Pubkey,
    ) -> Result<(), ProgramError>;
}
