//! LazorKit Error Types

use pinocchio::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum LazorKitError {
    #[error("Invalid instruction")]
    InvalidInstruction,

    #[error("Not authorized")]
    Unauthorized,

    #[error("Wallet already initialized")]
    AlreadyInitialized,

    #[error("Authority not found")]
    AuthorityNotFound,

    #[error("Policy verification failed")]
    PolicyVerificationFailed,

    #[error("Invalid wallet account")]
    InvalidWalletAccount,

    #[error("Account data too small")]
    AccountDataTooSmall,

    #[error("Policy did not return data")]
    PolicyReturnDataMissing,

    #[error("Invalid policy response")]
    InvalidPolicyResponse,

    #[error("Policy state size changed")]
    PolicyStateSizeChanged,

    #[error("Invalid session duration")]
    InvalidSessionDuration,

    #[error("Policy not found in registry")]
    UnverifiedPolicy,

    #[error("Policy has been deactivated")]
    PolicyDeactivated,

    #[error("Invalid PDA derivation")]
    InvalidPDA,
}

impl From<LazorKitError> for ProgramError {
    fn from(e: LazorKitError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
