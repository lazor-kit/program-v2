//! State crate for Lazorkit V2 wallet.
//!
//! This crate defines the state structures and logic for the Lazorkit V2 wallet system.

pub mod authority;
pub mod plugin;
pub mod plugin_ref;
pub mod position;
pub mod role_permission;
pub mod transmute;
pub mod wallet_account;

// Re-export AuthorityType from authority module
pub use authority::AuthorityType;

pub use no_padding::NoPadding;

// Re-export transmute traits
use pinocchio::program_error::ProgramError;
pub use transmute::{IntoBytes, Transmutable, TransmutableMut};

/// Discriminator for Lazorkit account types.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discriminator {
    /// Uninitialized account
    Uninitialized = 0,
    /// Wallet Account (main account)
    WalletAccount = 1,
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
#[repr(u32)]
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
    fn from(e: LazorkitAuthenticateError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
