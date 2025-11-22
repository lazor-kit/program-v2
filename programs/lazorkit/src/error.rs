use anchor_lang::error_code;

/// Error definitions for the LazorKit smart wallet program
///
/// Defines all possible error conditions that can occur during smart wallet
/// operations, providing clear error messages for debugging and user feedback.
/// Errors are organized by category for better maintainability.
#[error_code]
pub enum LazorKitError {
    // === Authentication & Passkey Errors ===
    #[msg("Passkey public key mismatch with stored authenticator")]
    PasskeyMismatch,

    #[msg("Invalid policy data size")]
    InvalidPolicyDataSize,

    // === Signature Verification Errors ===
    #[msg("Secp256r1 instruction has invalid data length")]
    Secp256r1InvalidLength,
    #[msg("Secp256r1 instruction header validation failed")]
    Secp256r1HeaderMismatch,
    #[msg("Secp256r1 signature data validation failed")]
    Secp256r1DataMismatch,
    #[msg("Invalid signature provided for passkey verification")]
    InvalidSignature,

    // === Client Data & Challenge Errors ===
    #[msg("Client data JSON is not valid UTF-8")]
    ClientDataInvalidUtf8,
    #[msg("Client data JSON parsing failed")]
    ClientDataJsonParseError,
    #[msg("Challenge field missing from client data JSON")]
    ChallengeMissing,
    #[msg("Challenge base64 decoding failed")]
    ChallengeBase64DecodeError,
    #[msg("Challenge message deserialization failed")]
    ChallengeDeserializationError,

    // === Timestamp & Nonce Errors ===
    #[msg("Message hash mismatch: expected different value")]
    HashMismatch,

    // === Policy Program Errors ===
    #[msg("Invalid instruction discriminator")]
    InvalidInstructionDiscriminator,

    // === Account & CPI Errors ===
    #[msg("Insufficient remaining accounts for CPI instruction")]
    InsufficientCpiAccounts,
    #[msg("Account slice index out of bounds")]
    AccountSliceOutOfBounds,

    // === Validation Errors ===
    #[msg("Account owner verification failed")]
    InvalidAccountOwner,

    // === Program Errors ===
    #[msg("Program not executable")]
    ProgramNotExecutable,

    // === Security Errors ===
    #[msg("Credential ID cannot be empty")]
    CredentialIdEmpty,
    #[msg("Policy data exceeds maximum allowed size")]
    PolicyDataTooLarge,
    #[msg("Transaction is too old")]
    TransactionTooOld,
    #[msg("Invalid instruction data")]
    InvalidInstructionData,
    #[msg("Invalid instruction")]
    InvalidInstruction,
    #[msg("Insufficient balance for fee")]
    InsufficientBalanceForFee,
    #[msg("Invalid sequence number")]
    InvalidSequenceNumber,
    #[msg("Invalid passkey format")]
    InvalidPasskeyFormat,
    #[msg("Reentrancy detected")]
    ReentrancyDetected,

    // === Admin Errors ===
    #[msg("Unauthorized admin")]
    UnauthorizedAdmin,
}
