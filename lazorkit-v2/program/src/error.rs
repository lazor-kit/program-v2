//! Error types for the Lazorkit V2 wallet program.

use pinocchio::program_error::ProgramError;

/// Custom error types for the Lazorkit V2 wallet program.
#[derive(Debug)]
#[repr(u32)]
pub enum LazorkitError {
    /// Invalid discriminator in WalletState account data
    InvalidWalletStateDiscriminator = 0,
    /// WalletState account owner does not match expected value
    OwnerMismatchWalletState,
    /// WalletState account is not empty when it should be
    AccountNotEmptyWalletState,
    /// Expected WalletState account to be a signer but it isn't
    ExpectedSignerWalletState,
    /// General state error in program execution
    StateError,
    /// Failed to borrow account data
    AccountBorrowFailed,
    /// Invalid authority type specified
    InvalidAuthorityType,
    /// Error during cross-program invocation
    Cpi,
    /// Invalid seed used for WalletState account derivation
    InvalidSeedWalletState,
    /// Required instructions are missing
    MissingInstructions,
    /// Invalid authority payload format
    InvalidAuthorityPayload,
    /// Authority not found for given role ID
    InvalidAuthorityNotFoundByRoleId,
    /// Error during instruction execution
    InstructionExecutionError,
    /// Error during data serialization
    SerializationError,
    /// Sign instruction data is too short
    InvalidSignInstructionDataTooShort,
    /// Create instruction data is too short
    InvalidCreateInstructionDataTooShort,
    /// Invalid number of accounts provided
    InvalidAccountsLength,
    /// WalletState account must be the first account in the list
    InvalidAccountsWalletStateMustBeFirst,
    /// Invalid system program account
    InvalidSystemProgram,
    /// Authority already exists
    DuplicateAuthority,
    /// Invalid operation attempted
    InvalidOperation,
    /// Data alignment error
    InvalidAlignment,
    /// Insufficient funds for operation
    InsufficientFunds,
    /// Permission denied for operation
    PermissionDenied,
    /// Invalid signature provided
    InvalidSignature,
    /// Instruction data is too short
    InvalidInstructionDataTooShort,
    /// Add authority instruction data is too short
    InvalidAddAuthorityInstructionDataTooShort,
    /// Plugin check failed
    PluginCheckFailed,
    /// Plugin not found
    PluginNotFound,
    /// Invalid plugin entry
    InvalidPluginEntry,
    /// Invalid create session instruction data too short
    InvalidCreateSessionInstructionDataTooShort,
    /// Debug: AddPlugin instruction data length check failed
    DebugAddPluginDataLength,
    /// Debug: AddPlugin pubkey parse failed
    DebugAddPluginPubkeyParse,
    /// Debug: AddPlugin plugin_registry_offset failed
    DebugAddPluginRegistryOffset,
    /// Debug: AddPlugin get_plugins failed
    DebugAddPluginGetPlugins,
    /// Debug: process_action instruction_data empty
    DebugProcessActionEmpty,
    /// Debug: process_action instruction_data too short
    DebugProcessActionTooShort,
    /// Debug: process_action instruction_u16 value
    DebugProcessActionU16,
    /// Debug: process_action instruction not matched
    DebugProcessActionNotMatched,
}

impl From<LazorkitError> for ProgramError {
    fn from(e: LazorkitError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
