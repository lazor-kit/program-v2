//! LazorKit State Module
//!
//! This module defines the core state structures for the LazorKit smart wallet.

pub mod authority;
pub mod builder;
pub mod error;
pub mod transmute;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

pub use authority::ed25519::{Ed25519Authority, Ed25519SessionAuthority};
pub use authority::secp256r1::{Secp256r1Authority, Secp256r1SessionAuthority};

pub use authority::{AuthorityInfo, AuthorityType};
pub use builder::LazorKitBuilder;
pub use error::{LazorAuthenticateError, LazorStateError};
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
///
/// Memory layout (16 bytes):
/// - authority_type: u16 (2 bytes)
/// - authority_length: u16 (2 bytes)
/// - role_type: u8 (1 byte) - 0=Owner, 1=Admin, 2=Spender
/// - padding: [u8; 3] (3 bytes)
/// - id: u32 (4 bytes)
/// - boundary: u32 (4 bytes)
#[repr(C, align(8))]
#[derive(Debug, PartialEq, Copy, Clone, no_padding::NoPadding)]
pub struct Position {
    /// Type of authority (see AuthorityType enum)
    pub authority_type: u16,
    /// Length of authority data in bytes
    pub authority_length: u16,
    /// Role type: 0=Owner, 1=Admin, 2=Spender
    pub role_type: u8,
    /// Padding for 8-byte alignment
    _padding: [u8; 3],
    /// Unique role ID
    pub id: u32,
    /// Absolute offset to the next role (boundary)
    pub boundary: u32,
}

impl Position {
    pub const LEN: usize = 16;

    pub fn new(authority_type: AuthorityType, authority_length: u16, id: u32) -> Self {
        // Determine role type based on ID for backwards compatibility
        let role_type = if id == 0 {
            0 // Owner
        } else if id == 1 {
            1 // Admin
        } else {
            2 // Spender
        };

        Self {
            authority_type: authority_type as u16,
            authority_length,
            role_type,
            _padding: [0; 3],
            id,
            boundary: 0, // Will be set during serialization
        }
    }

    /// Returns the role type (0=Owner, 1=Admin, 2=Spender)
    pub fn role_type(&self) -> u8 {
        self.role_type
    }

    /// Checks if this position has admin or owner privileges
    pub fn is_admin_or_owner(&self) -> bool {
        self.role_type == 0 || self.role_type == 1
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
    type Item = Result<(Position, &'a [u8]), ProgramError>; // Return Result instead of just value

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        if self.cursor + Position::LEN > self.buffer.len() {
            return Some(Err(ProgramError::InvalidAccountData)); // Error instead of None
        }

        let position = match read_position(&self.buffer[self.cursor..]) {
            Ok(pos) => *pos,
            Err(e) => return Some(Err(e)), // Propagate error
        };

        let authority_start = self.cursor + Position::LEN;
        let authority_end = authority_start + position.authority_length as usize;

        // Validate boundary
        if position.boundary as usize > self.buffer.len() {
            return Some(Err(ProgramError::InvalidAccountData)); // Error instead of None
        }

        if authority_end > self.buffer.len() {
            return Some(Err(ProgramError::InvalidAccountData)); // Error instead of None
        }

        let authority_data = &self.buffer[authority_start..authority_end];

        self.cursor = position.boundary as usize;
        self.remaining -= 1;

        Some(Ok((position, authority_data)))
    }
}
