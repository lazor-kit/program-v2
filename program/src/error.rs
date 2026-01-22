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
}

impl From<AuthError> for ProgramError {
    fn from(e: AuthError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
