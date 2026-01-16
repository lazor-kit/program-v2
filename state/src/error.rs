use pinocchio::program_error::ProgramError;

/// Error types related to authentication operations.
pub enum LazorAuthenticateError {
    /// Invalid authority provided
    InvalidAuthority = 3000,
    /// Invalid authority payload format
    InvalidAuthorityPayload,
    /// Invalid data payload format
    InvalidDataPayload,
    /// Missing Ed25519 authority account
    InvalidAuthorityEd25519MissingAuthorityAccount,
    /// Authority does not support session-based authentication
    AuthorityDoesNotSupportSessionBasedAuth,
    /// Generic permission denied error
    PermissionDenied,
    /// Missing required permission
    PermissionDeniedMissingPermission,
    /// Token account permission check failed
    PermissionDeniedTokenAccountPermissionFailure,
    /// Token account has an active delegate or close authority
    PermissionDeniedTokenAccountDelegatePresent,
    /// Token account is not initialized
    PermissionDeniedTokenAccountNotInitialized,
    /// No permission to manage authority
    PermissionDeniedToManageAuthority,
    /// Insufficient balance for operation
    PermissionDeniedInsufficientBalance,
    /// Cannot remove root authority
    PermissionDeniedCannotRemoveRootAuthority,
    /// Cannot update root authority
    PermissionDeniedCannotUpdateRootAuthority,
    /// Session has expired
    PermissionDeniedSessionExpired,
    /// Invalid Secp256k1 signature
    PermissionDeniedSecp256k1InvalidSignature,
    /// Secp256k1 signature age is invalid
    PermissionDeniedSecp256k1InvalidSignatureAge,
    /// Secp256k1 signature has been reused
    PermissionDeniedSecp256k1SignatureReused,
    /// Invalid Secp256k1 hash
    PermissionDeniedSecp256k1InvalidHash,
    /// Secp256r1 signature has been reused
    PermissionDeniedSecp256r1SignatureReused,
    /// Stake account is in an invalid state
    PermissionDeniedStakeAccountInvalidState,
    /// Cannot reuse session key
    InvalidSessionKeyCannotReuseSessionKey,
    /// Invalid session duration
    InvalidSessionDuration,
    /// Token account authority is not the Lazor account
    PermissionDeniedTokenAccountAuthorityNotLazor,
    /// Invalid Secp256r1 instruction
    PermissionDeniedSecp256r1InvalidInstruction,
    /// Invalid Secp256r1 public key
    PermissionDeniedSecp256r1InvalidPubkey,
    /// Invalid Secp256r1 message hash
    PermissionDeniedSecp256r1InvalidMessageHash,
    /// Invalid Secp256r1 message
    PermissionDeniedSecp256r1InvalidMessage,
    /// Invalid Secp256r1 authentication kind
    PermissionDeniedSecp256r1InvalidAuthenticationKind,
    /// SOL destination limit exceeded
    PermissionDeniedSolDestinationLimitExceeded,
    /// SOL destination recurring limit exceeded
    PermissionDeniedSolDestinationRecurringLimitExceeded,
    /// Token destination limit exceeded
    PermissionDeniedTokenDestinationLimitExceeded,
    /// Token destination recurring limit exceeded
    PermissionDeniedRecurringTokenDestinationLimitExceeded,
    /// Program execution instruction is invalid
    PermissionDeniedProgramExecInvalidInstruction,
    /// Program execution program ID does not match
    PermissionDeniedProgramExecInvalidProgram,
    /// Program execution instruction data does not match prefix
    PermissionDeniedProgramExecInvalidInstructionData,
    /// Program execution missing required accounts
    PermissionDeniedProgramExecMissingAccounts,
    /// Program execution config account index mismatch
    PermissionDeniedProgramExecInvalidConfigAccount,
    /// Program execution wallet account index mismatch
    PermissionDeniedProgramExecInvalidWalletAccount,
    /// Program execution cannot be the Lazor program
    PermissionDeniedProgramExecCannotBeLazor,
}

impl From<LazorAuthenticateError> for ProgramError {
    fn from(e: LazorAuthenticateError) -> Self {
        ProgramError::Custom(e as u32 + 1000) // Base offset to avoid collision
    }
}

/// Error types related to state management operations.
pub enum LazorStateError {
    /// Account data is invalid or corrupted
    InvalidAccountData = 1000,
    /// Action data is invalid or malformed
    InvalidActionData,
    /// Authority data is invalid or malformed
    InvalidAuthorityData,
    /// Role data is invalid or malformed
    InvalidRoleData,
    /// Lazor account data is invalid or malformed
    InvalidLazorData,
    /// Specified role could not be found
    RoleNotFound,
    /// Error loading permissions
    PermissionLoadError,
    /// Adding an authority requires at least one policy
    InvalidAuthorityMustHaveAtLeastOnePolicy,
}

impl From<LazorStateError> for ProgramError {
    fn from(e: LazorStateError) -> Self {
        ProgramError::Custom(e as u32 + 2000)
    }
}
