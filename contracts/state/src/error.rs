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
    /// Invalid Secp256r1 signature
    PermissionDeniedSecp256r1InvalidSignature,
    /// Secp256r1 signature age is invalid
    PermissionDeniedSecp256r1InvalidSignatureAge,
    /// Secp256r1 signature has been reused
    PermissionDeniedSecp256r1SignatureReused,
    /// Invalid Secp256r1 hash
    PermissionDeniedSecp256r1InvalidHash,
    /// Cannot reuse session key
    InvalidSessionKeyCannotReuseSessionKey,
    /// Invalid session duration
    InvalidSessionDuration,
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
    /// Authority data is invalid or malformed
    InvalidAuthorityData,
    /// Role data is invalid or malformed
    InvalidRoleData,
    /// Specified role could not be found
    RoleNotFound,
}

impl From<LazorStateError> for ProgramError {
    fn from(e: LazorStateError) -> Self {
        ProgramError::Custom(e as u32 + 2000)
    }
}
