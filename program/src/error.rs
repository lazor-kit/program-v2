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

    #[error("Plugin verification failed")]
    PluginVerificationFailed,

    #[error("Invalid wallet account")]
    InvalidWalletAccount,

    #[error("Account data too small")]
    AccountDataTooSmall,

    #[error("Plugin did not return data")]
    PluginReturnDataMissing,

    #[error("Invalid plugin response")]
    InvalidPluginResponse,

    #[error("Plugin state size changed")]
    PluginStateSizeChanged,

    #[error("Invalid session duration")]
    InvalidSessionDuration,
}

impl From<LazorKitError> for ProgramError {
    fn from(e: LazorKitError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
