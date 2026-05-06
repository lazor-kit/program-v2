use pinocchio::program_error::ProgramError;

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum AuthError {
    InvalidAuthorityPayload = 3001,
    PermissionDenied = 3002,
    InvalidInstruction = 3003,
    InvalidPubkey = 3004,
    InvalidMessageHash = 3005,
    SignatureReused = 3006,
    InvalidSignatureAge = 3007,
    InvalidSessionDuration = 3008,
    SessionExpired = 3009,
    AuthorityDoesNotSupportSession = 3010,
    InvalidAuthenticationKind = 3011,
    InvalidMessage = 3012,
    SelfReentrancyNotAllowed = 3013,
    DeferredAuthorizationExpired = 3014,
    DeferredHashMismatch = 3015,
    InvalidExpiryWindow = 3016,
    UnauthorizedReclaim = 3017,
    DeferredAuthorizationNotExpired = 3018,
    InvalidSessionAccount = 3019,
    // Session action errors (codes aligned with lazorkit-protocol for unified SDK error decoding)
    ActionBufferInvalid = 3020,
    ActionProgramNotWhitelisted = 3021,
    ActionProgramBlacklisted = 3022,
    ActionSolMaxPerTxExceeded = 3023,
    ActionSolLimitExceeded = 3024,
    ActionSolRecurringLimitExceeded = 3025,
    ActionTokenLimitExceeded = 3026,
    ActionTokenRecurringLimitExceeded = 3027,
    ActionWhitelistBlacklistConflict = 3028,
    ActionTokenMaxPerTxExceeded = 3029,
    // Session vault + token invariants (defense against System::Assign / SetAuthority escapes)
    SessionVaultOwnerChanged = 3030,
    SessionVaultDataLenChanged = 3031,
    SessionTokenAuthorityChanged = 3032,
}

impl From<AuthError> for ProgramError {
    fn from(e: AuthError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
