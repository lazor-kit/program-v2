//! LazorKit State Module
//!
//! This module defines the core state structures for the LazorKit smart wallet.
//! Implements the Swig-compatible architecture with Plugin-based permissions.

pub mod authority;
pub mod builder;
pub mod error;
pub mod policy;
pub mod registry;
pub mod transmute;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

pub use authority::ed25519::{Ed25519Authority, Ed25519SessionAuthority};
pub use authority::programexec::{ProgramExecAuthority, ProgramExecSessionAuthority};
pub use authority::secp256k1::{Secp256k1Authority, Secp256k1SessionAuthority};
pub use authority::secp256r1::{Secp256r1Authority, Secp256r1SessionAuthority};

pub use authority::{AuthorityInfo, AuthorityType};
pub use builder::LazorKitBuilder;
pub use error::{LazorAuthenticateError, LazorStateError};
pub use policy::PolicyHeader;
pub use registry::PolicyRegistryEntry;
pub use transmute::{IntoBytes, Transmutable, TransmutableMut};

/// Represents the type discriminator for different account types in the system.
#[repr(u8)]
pub enum Discriminator {
    /// LazorKit wallet config account
    LazorKitWallet = 1,
}

impl From<u8> for Discriminator {
    fn from(discriminator: u8) -> Self {
        match discriminator {
            1 => Discriminator::LazorKitWallet,
            _ => panic!("Invalid discriminator"),
        }
    }
}

/// Main LazorKit wallet account structure (Header)
/// This is the "Config" account that stores RBAC configuration.
///
/// PDA Seeds: ["lazorkit", id]
#[repr(C, align(8))]
#[derive(Debug, Copy, Clone, no_padding::NoPadding)]
pub struct LazorKitWallet {
    /// Account type discriminator (= 1)
    pub discriminator: u8,

    /// PDA bump seed
    pub bump: u8,

    /// Unique wallet ID (32 bytes, used for PDA derivation)
    pub id: [u8; 32],

    /// Number of active roles
    pub role_count: u16,

    /// Counter for generating unique role IDs (auto-increment)
    pub role_counter: u32,

    /// Bump seed for WalletAddress (Vault)
    pub wallet_bump: u8,

    /// Reserved for future use
    pub reserved: [u8; 7],
}

impl LazorKitWallet {
    /// Header size: 1 + 1 + 32 + 2 + 4 + 1 + 7 = 48 bytes
    pub const LEN: usize = 48;

    /// Creates a new LazorKit wallet header
    pub fn new(id: [u8; 32], bump: u8, wallet_bump: u8) -> Self {
        Self {
            discriminator: Discriminator::LazorKitWallet as u8,
            bump,
            id,
            role_count: 0,
            role_counter: 0,
            wallet_bump,
            reserved: [0; 7],
        }
    }

    /// Validate discriminator
    pub fn is_valid(&self) -> bool {
        self.discriminator == Discriminator::LazorKitWallet as u8
    }
}

impl Transmutable for LazorKitWallet {
    const LEN: usize = core::mem::size_of::<LazorKitWallet>();
}

impl TransmutableMut for LazorKitWallet {}

impl IntoBytes for LazorKitWallet {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        Ok(unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) })
    }
}

/// Position header for a Role in the dynamic buffer.
/// This matches Swig's Position structure.
///
/// Memory layout (16 bytes):
/// - authority_type: u16 (2 bytes)
/// - authority_length: u16 (2 bytes)
/// - num_policies: u16 (2 bytes)
/// - padding: u16 (2 bytes)
/// - id: u32 (4 bytes)
/// - boundary: u32 (4 bytes)
#[repr(C, align(8))]
#[derive(Debug, PartialEq, Copy, Clone, no_padding::NoPadding)]
pub struct Position {
    /// Type of authority (see AuthorityType enum)
    pub authority_type: u16,

    /// Length of authority data in bytes
    pub authority_length: u16,

    /// Number of policies attached to this role
    pub num_policies: u16,

    /// Padding for alignment
    pub padding: u16,

    /// Unique role ID
    pub id: u32,

    /// Absolute offset to the next role (boundary)
    pub boundary: u32,
}

impl Position {
    pub const LEN: usize = 16;

    pub fn new(
        authority_type: AuthorityType,
        authority_length: u16,
        num_policies: u16,
        id: u32,
    ) -> Self {
        Self {
            authority_type: authority_type as u16,
            authority_length,
            num_policies,
            padding: 0,
            id,
            boundary: 0, // Will be set during serialization
        }
    }
}

impl Transmutable for Position {
    const LEN: usize = core::mem::size_of::<Position>();
}

impl TransmutableMut for Position {}

impl IntoBytes for Position {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        Ok(unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) })
    }
}

/// Generate PDA seeds for a LazorKit config wallet
pub fn wallet_seeds(id: &[u8]) -> [&[u8]; 2] {
    [b"lazorkit", id]
}

/// Generate PDA seeds with bump
pub fn wallet_seeds_with_bump<'a>(id: &'a [u8], bump: &'a [u8]) -> [&'a [u8]; 3] {
    [b"lazorkit", id, bump]
}

/// Generate PDA seeds for WalletAddress (Vault)
pub fn vault_seeds(config_key: &Pubkey) -> [&[u8]; 2] {
    [b"lazorkit-wallet-address", config_key.as_ref()]
}

/// Generate vault PDA seeds with bump
pub fn vault_seeds_with_bump<'a>(config_key: &'a Pubkey, bump: &'a [u8]) -> [&'a [u8]; 3] {
    [b"lazorkit-wallet-address", config_key.as_ref(), bump]
}

/// Helper to read a Position from a byte slice
pub fn read_position(data: &[u8]) -> Result<&Position, ProgramError> {
    if data.len() < Position::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    unsafe { Position::load_unchecked(&data[..Position::LEN]) }
}

/// Helper to iterate through roles in a buffer
pub struct RoleIterator<'a> {
    buffer: &'a [u8],
    cursor: usize,
    remaining: u16,
}

impl<'a> RoleIterator<'a> {
    pub fn new(buffer: &'a [u8], role_count: u16, start_offset: usize) -> Self {
        Self {
            buffer,
            cursor: start_offset,
            remaining: role_count,
        }
    }
}

impl<'a> Iterator for RoleIterator<'a> {
    type Item = (Position, &'a [u8], &'a [u8]); // (header, authority_data, policies_data)

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        if self.cursor + Position::LEN > self.buffer.len() {
            return None;
        }

        let position = *read_position(&self.buffer[self.cursor..]).ok()?;

        let authority_start = self.cursor + Position::LEN;
        let authority_end = authority_start + position.authority_length as usize;
        let policies_end = position.boundary as usize;

        if policies_end > self.buffer.len() {
            return None;
        }

        let authority_data = &self.buffer[authority_start..authority_end];
        let policies_data = &self.buffer[authority_end..policies_end];

        self.cursor = policies_end;
        self.remaining -= 1;

        Some((position, authority_data, policies_data))
    }
}
