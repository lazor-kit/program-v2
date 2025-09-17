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
    #[msg("Smart wallet address mismatch with authenticator")]
    SmartWalletDataMismatch,

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
    #[msg("Message timestamp is too far in the past")]
    TimestampTooOld,
    #[msg("Message timestamp is too far in the future")]
    TimestampTooNew,
    #[msg("Nonce mismatch: expected different value")]
    NonceMismatch,
    #[msg("Nonce overflow: cannot increment further")]
    NonceOverflow,

    // === Policy Program Errors ===
    #[msg("Policy program not found in registry")]
    PolicyProgramNotRegistered,
    #[msg("The policy program registry is full.")]
    WhitelistFull,
    #[msg("Invalid instruction discriminator for check_policy")]
    InvalidCheckPolicyDiscriminator,
    #[msg("Invalid instruction discriminator for destroy")]
    InvalidDestroyDiscriminator,
    #[msg("Invalid instruction discriminator for init_policy")]
    InvalidInitPolicyDiscriminator,
    #[msg("Old and new policy programs are identical")]
    PolicyProgramsIdentical,
    #[msg("Neither old nor new policy program is the default")]
    NoDefaultPolicyProgram,
    #[msg("Policy program already registered")]
    PolicyProgramAlreadyRegistered,

    // === Account & CPI Errors ===
    #[msg("Invalid remaining accounts")]
    InvalidRemainingAccounts,
    #[msg("CPI data is required but not provided")]
    CpiDataMissing,
    #[msg("Insufficient remaining accounts for policy instruction")]
    InsufficientPolicyAccounts,
    #[msg("Insufficient remaining accounts for CPI instruction")]
    InsufficientCpiAccounts,
    #[msg("Account slice index out of bounds")]
    AccountSliceOutOfBounds,

    // === Financial Errors ===
    #[msg("Transfer amount would cause arithmetic overflow")]
    TransferAmountOverflow,

    // === Validation Errors ===
    #[msg("Invalid bump seed for PDA derivation")]
    InvalidBumpSeed,
    #[msg("Account owner verification failed")]
    InvalidAccountOwner,

    // === Program Errors ===
    #[msg("Program not executable")]
    ProgramNotExecutable,
    #[msg("Program is paused")]
    ProgramPaused,
    #[msg("Wallet device already initialized")]
    WalletDeviceAlreadyInitialized,

    // === Security Errors ===
    #[msg("Credential ID exceeds maximum allowed size")]
    CredentialIdTooLarge,
    #[msg("Credential ID cannot be empty")]
    CredentialIdEmpty,
    #[msg("Policy data exceeds maximum allowed size")]
    PolicyDataTooLarge,
    #[msg("CPI data exceeds maximum allowed size")]
    CpiDataTooLarge,
    #[msg("Too many remaining accounts provided")]
    TooManyRemainingAccounts,
    #[msg("Invalid PDA derivation")]
    InvalidPDADerivation,
    #[msg("Transaction is too old")]
    TransactionTooOld,
    #[msg("Invalid account data")]
    InvalidAccountData,
    #[msg("Invalid instruction data")]
    InvalidInstructionData,
    #[msg("Account already initialized")]
    AccountAlreadyInitialized,
    #[msg("Invalid account state")]
    InvalidAccountState,
    #[msg("Invalid fee amount")]
    InvalidFeeAmount,
    #[msg("Insufficient balance for fee")]
    InsufficientBalanceForFee,
    #[msg("Invalid authority")]
    InvalidAuthority,
    #[msg("Authority mismatch")]
    AuthorityMismatch,
    #[msg("Invalid sequence number")]
    InvalidSequenceNumber,
    #[msg("Invalid passkey format")]
    InvalidPasskeyFormat,
    #[msg("Invalid message format")]
    InvalidMessageFormat,
    #[msg("Invalid split index")]
    InvalidSplitIndex,
    #[msg("Invalid program address")]
    InvalidProgramAddress,
    #[msg("Reentrancy detected")]
    ReentrancyDetected,

    // === Vault Errors ===
    #[msg("Invalid vault index")]
    InvalidVaultIndex,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Invalid action")]
    InvalidAction,
    #[msg("Insufficient balance in vault")]
    InsufficientVaultBalance,
}
