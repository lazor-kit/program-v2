//! State crate for Lazorkit V2 wallet.
//!
//! This crate defines the state structures and logic for the Lazorkit V2 wallet system.

pub mod wallet_account;
pub mod wallet_authority;
pub mod position;
pub mod plugin;
pub mod plugin_ref;
pub mod transmute;
pub mod authority;

// Re-export AuthorityType from authority module
pub use authority::AuthorityType;

pub use no_padding::NoPadding;

// Re-export transmute traits
pub use transmute::{Transmutable, TransmutableMut, IntoBytes};
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

/// Discriminator for Lazorkit account types.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discriminator {
    /// Uninitialized account
    Uninitialized = 0,
    /// Wallet Account (main account, Swig-like)
    WalletAccount = 1,
    /// Wallet Authority account (legacy, may be removed)
    WalletAuthority = 2,
}

/// Account classification for automatic account detection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccountClassification {
    /// This is the Lazorkit WalletAccount (first account)
    ThisLazorkitConfig { lamports: u64 },
    /// This is the Lazorkit wallet vault address (System-owned PDA)
    LazorkitWalletAddress { lamports: u64 },
    /// This is a token account owned by the Lazorkit wallet
    LazorkitTokenAccount { owner: Pubkey, mint: Pubkey, amount: u64 },
    /// This is a stake account with the Lazorkit wallet as withdrawer
    LazorkitStakeAccount { balance: u64 },
    /// Not a special account
    None,
}

/// Error type for state-related operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LazorkitStateError {
    /// Invalid account data
    InvalidAccountData,
    /// Invalid discriminator
    InvalidDiscriminator,
    /// Invalid authority type
    InvalidAuthorityType,
    /// Invalid authority data
    InvalidAuthorityData,
    /// Wallet state not found
    WalletStateNotFound,
    /// Wallet authority not found
    WalletAuthorityNotFound,
    /// Plugin not found
    PluginNotFound,
    /// Invalid plugin entry
    InvalidPluginEntry,
    /// Invalid role data
    InvalidRoleData,
}

impl From<LazorkitStateError> for ProgramError {
    fn from(e: LazorkitStateError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

/// Error type for authentication operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LazorkitAuthenticateError {
    /// Invalid signature
    InvalidSignature,
    /// Invalid recovery id
    InvalidRecoveryId,
    /// Invalid format
    InvalidFormat,
    /// PermissionDeniedSecp256k1InvalidSignatureAge
    PermissionDeniedSecp256k1InvalidSignatureAge,
    /// Invalid authority payload
    InvalidAuthorityPayload,
    /// Invalid session duration
    InvalidSessionDuration,
    /// Session expired
    PermissionDeniedSessionExpired,
    /// Permission denied
    PermissionDenied,
    /// Missing authority account for Ed25519
    InvalidAuthorityEd25519MissingAuthorityAccount,
    /// Secp256k1 Signature reused
    PermissionDeniedSecp256k1SignatureReused,
    /// Secp256k1 Invalid signature
    PermissionDeniedSecp256k1InvalidSignature,
    /// Secp256k1 Invalid hash
    PermissionDeniedSecp256k1InvalidHash,
    /// Secp256r1 Invalid instruction
    PermissionDeniedSecp256r1InvalidInstruction,
    /// Secp256r1 Signature reused
    PermissionDeniedSecp256r1SignatureReused,
    /// Secp256r1 Invalid pubkey
    PermissionDeniedSecp256r1InvalidPubkey,
    /// Secp256r1 Invalid message hash
    PermissionDeniedSecp256r1InvalidMessageHash,
    /// Secp256r1 Invalid message
    PermissionDeniedSecp256r1InvalidMessage,
    /// Secp256r1 Invalid authentication kind
    PermissionDeniedSecp256r1InvalidAuthenticationKind,
    /// Authority does not support session based auth
    AuthorityDoesNotSupportSessionBasedAuth,
    /// Program execution cannot be lazorkit
    PermissionDeniedProgramExecCannotBeLazorkit,
    /// Program execution invalid instruction
    PermissionDeniedProgramExecInvalidInstruction,
    /// Program execution invalid instruction data
    PermissionDeniedProgramExecInvalidInstructionData,
    /// Program execution invalid program
    PermissionDeniedProgramExecInvalidProgram,
    /// Program execution invalid wallet account
    PermissionDeniedProgramExecInvalidWalletAccount,
    /// Program execution invalid config account
    PermissionDeniedProgramExecInvalidConfigAccount,
}

impl From<LazorkitAuthenticateError> for ProgramError {
    fn from(_e: LazorkitAuthenticateError) -> Self {
        ProgramError::InvalidAccountData
    }
}
