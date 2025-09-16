use anchor_lang::error_code;

/// Custom errors for the Lazor Kit program
#[error_code]
pub enum LazorKitError {
    // === Authentication & Passkey Errors ===
    #[msg("Passkey public key mismatch with stored authenticator")]
    PasskeyMismatch,
    #[msg("Smart wallet address mismatch with authenticator")]
    SmartWalletDataMismatch,
    #[msg("Smart wallet authenticator account not found or invalid")]
    AuthenticatorNotFound,

    // === Signature Verification Errors ===
    #[msg("Secp256r1 instruction has invalid data length")]
    Secp256r1InvalidLength,
    #[msg("Secp256r1 instruction header validation failed")]
    Secp256r1HeaderMismatch,
    #[msg("Secp256r1 signature data validation failed")]
    Secp256r1DataMismatch,
    #[msg("Secp256r1 instruction not found at specified index")]
    Secp256r1InstructionNotFound,
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
    #[msg("Policy data is required but not provided")]
    PolicyDataRequired,
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

    // === Account & CPI Errors ===
    #[msg("Invalid remaining accounts")]
    InvalidRemainingAccounts,
    #[msg("CPI data is required but not provided")]
    CpiDataMissing,
    #[msg("CPI data is invalid or malformed")]
    InvalidCpiData,
    #[msg("Insufficient remaining accounts for policy instruction")]
    InsufficientPolicyAccounts,
    #[msg("Insufficient remaining accounts for CPI instruction")]
    InsufficientCpiAccounts,
    #[msg("Account slice index out of bounds")]
    AccountSliceOutOfBounds,
    #[msg("SOL transfer requires at least 2 remaining accounts")]
    SolTransferInsufficientAccounts,
    #[msg("New authenticator account is required but not provided")]
    NewWalletDeviceMissing,
    #[msg("New authenticator passkey is required but not provided")]
    NewWalletDevicePasskeyMissing,

    // === Financial Errors ===
    #[msg("Insufficient lamports for requested transfer")]
    InsufficientLamports,
    #[msg("Transfer amount would cause arithmetic overflow")]
    TransferAmountOverflow,

    // === Validation Errors ===
    #[msg("Invalid bump seed for PDA derivation")]
    InvalidBumpSeed,
    #[msg("Account owner verification failed")]
    InvalidAccountOwner,
    #[msg("Account discriminator mismatch")]
    InvalidAccountDiscriminator,

    // === Program Errors ===
    #[msg("Invalid program ID")]
    InvalidProgramId,
    #[msg("Program not executable")]
    ProgramNotExecutable,
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
    #[msg("Rate limit exceeded")]
    RateLimitExceeded,
    #[msg("Invalid account data")]
    InvalidAccountData,
    #[msg("Unauthorized access attempt")]
    Unauthorized,
    #[msg("Program is paused")]
    ProgramPaused,
    #[msg("Invalid instruction data")]
    InvalidInstructionData,
    #[msg("Account already initialized")]
    AccountAlreadyInitialized,
    #[msg("Account not initialized")]
    AccountNotInitialized,
    #[msg("Invalid account state")]
    InvalidAccountState,
    #[msg("Operation would cause integer overflow")]
    IntegerOverflow,
    #[msg("Operation would cause integer underflow")]
    IntegerUnderflow,
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
    #[msg("Duplicate transaction detected")]
    DuplicateTransaction,
    #[msg("Invalid transaction ordering")]
    InvalidTransactionOrdering,
    #[msg("Maximum wallet limit reached")]
    MaxWalletLimitReached,
    #[msg("Invalid wallet configuration")]
    InvalidWalletConfiguration,
    #[msg("Wallet not found")]
    WalletNotFound,
    #[msg("Invalid passkey format")]
    InvalidPasskeyFormat,
    #[msg("Passkey already registered")]
    PasskeyAlreadyRegistered,
    #[msg("Invalid message format")]
    InvalidMessageFormat,
    #[msg("Message size exceeds limit")]
    MessageSizeExceedsLimit,
    #[msg("Invalid split index")]
    InvalidSplitIndex,
    #[msg("CPI execution failed")]
    CpiExecutionFailed,
    #[msg("Invalid program address")]
    InvalidProgramAddress,
    #[msg("Whitelist operation failed")]
    WhitelistOperationFailed,
    #[msg("Invalid whitelist state")]
    InvalidWhitelistState,
    #[msg("Emergency shutdown activated")]
    EmergencyShutdown,
    #[msg("Recovery mode required")]
    RecoveryModeRequired,
    #[msg("Invalid recovery attempt")]
    InvalidRecoveryAttempt,
    #[msg("Audit log full")]
    AuditLogFull,
    #[msg("Invalid audit entry")]
    InvalidAuditEntry,
    #[msg("Reentrancy detected")]
    ReentrancyDetected,
    #[msg("Invalid call depth")]
    InvalidCallDepth,
    #[msg("Stack overflow protection triggered")]
    StackOverflowProtection,
    #[msg("Memory limit exceeded")]
    MemoryLimitExceeded,
    #[msg("Computation limit exceeded")]
    ComputationLimitExceeded,
    #[msg("Invalid rent exemption")]
    InvalidRentExemption,
    #[msg("Account closure failed")]
    AccountClosureFailed,
    #[msg("Invalid account closure")]
    InvalidAccountClosure,
    #[msg("Refund failed")]
    RefundFailed,
    #[msg("Invalid refund amount")]
    InvalidRefundAmount,

    // === Vault Errors ===
    #[msg("All vault slots are full")]
    AllVaultsFull,
    #[msg("Vault not found for the specified mint")]
    VaultNotFound,
    #[msg("Insufficient balance in vault")]
    InsufficientVaultBalance,
    #[msg("Vault balance overflow")]
    VaultOverflow,
    #[msg("Invalid vault index")]
    InvalidVaultIndex,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Invalid action")]
    InvalidAction,
}
