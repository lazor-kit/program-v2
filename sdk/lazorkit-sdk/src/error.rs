use solana_sdk::pubkey::Pubkey;
use thiserror::Error;

/// SDK-specific error types for LazorKit operations
#[derive(Debug, Error)]
pub enum LazorSdkError {
    /// Connection or RPC error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Account not found on-chain
    #[error("Account not found: {0}")]
    AccountNotFound(Pubkey),

    /// Invalid account data or deserialization error
    #[error("Invalid account data: {0}")]
    InvalidAccountData(String),

    /// Role not found in wallet
    #[error("Role {0} not found in wallet")]
    RoleNotFound(u32),

    /// Wallet not initialized or invalid state
    #[error("Invalid wallet state: {0}")]
    InvalidWalletState(String),

    /// Borsh serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] std::io::Error),

    /// Program error from on-chain
    #[error("Program error: {0}")]
    ProgramError(#[from] solana_sdk::program_error::ProgramError),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

/// Result type alias for SDK operations
pub type Result<T> = std::result::Result<T, LazorSdkError>;
