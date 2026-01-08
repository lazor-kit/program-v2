//! Wallet Authority account structure.

use crate::{Transmutable, TransmutableMut, IntoBytes};
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};
use crate::authority::{AuthorityInfo, AuthorityType};
use crate::authority::ed25519::{ED25519Authority, Ed25519SessionAuthority};
use crate::authority::secp256k1::{Secp256k1Authority, Secp256k1SessionAuthority};
use crate::authority::secp256r1::{Secp256r1Authority, Secp256r1SessionAuthority};
use crate::authority::programexec::{ProgramExecAuthority, session::ProgramExecSessionAuthority};

/// Wallet Authority account structure.
///
/// This account links an authority (passkey, keypair, etc.) to a smart wallet.
#[repr(C, align(8))]
#[derive(Debug, PartialEq)]
pub struct WalletAuthority {
    pub discriminator: u8,         // Manual discriminator (2 = WalletAuthority)
    pub bump: u8,
    pub authority_type: u16,        // AuthorityType as u16
    pub smart_wallet: Pubkey,       // 32 bytes
    pub role_id: u32,               // 0 = no role
    pub _padding: [u8; 6],          // Padding to align to 8 bytes (42 + 6 = 48)
    
    // Dynamic: Authority data follows after this struct in account data
    // authority_data: Variable length based on authority_type
    // session_data: Optional, if session-based
}

impl WalletAuthority {
    /// Size of the fixed header (without dynamic authority data)
    pub const LEN: usize = core::mem::size_of::<Self>();
    
    /// PDA seed prefix for WalletAuthority
    pub const PREFIX_SEED: &'static [u8] = b"wallet_authority";
}

/// Helper functions for WalletAuthority PDA derivation
pub fn wallet_authority_seeds_with_bump<'a>(
    smart_wallet: &'a Pubkey,
    authority_hash: &'a [u8; 32],
    bump: &'a [u8],
) -> [&'a [u8]; 4] {
    [
        WalletAuthority::PREFIX_SEED,
        smart_wallet.as_ref(),
        authority_hash,
        bump,
    ]
}

/// Creates a signer seeds array for a WalletAuthority account.
pub fn wallet_authority_signer<'a>(
    smart_wallet: &'a Pubkey,
    authority_hash: &'a [u8; 32],
    bump: &'a [u8; 1],
) -> [pinocchio::instruction::Seed<'a>; 4] {
    [
        WalletAuthority::PREFIX_SEED.into(),
        smart_wallet.as_ref().into(),
        authority_hash.into(),
        bump.as_ref().into(),
    ]
}

impl WalletAuthority {
    /// Get authority data from account
    pub fn get_authority_data<'a>(&self, account_data: &'a [u8]) -> Result<&'a [u8], ProgramError> {
        if account_data.len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(&account_data[Self::LEN..])
    }
    
    /// Get authority type
    pub fn authority_type(&self) -> Result<AuthorityType, ProgramError> {
        AuthorityType::try_from(self.authority_type)
    }
    
    /// Check if authority is session-based
    pub fn is_session_based(&self) -> bool {
        matches!(
            self.authority_type,
            2 | 4 | 6 | 8  // Session variants
        )
    }
    
    /// Load authority as AuthorityInfo trait object from account data
    pub fn load_authority_info<'a>(
        &'a self,
        account_data: &'a [u8],
    ) -> Result<&'a dyn AuthorityInfo, ProgramError> {
        let authority_data = self.get_authority_data(account_data)?;
        let auth_type = self.authority_type()?;
        
        let authority: &dyn AuthorityInfo = match auth_type {
            AuthorityType::Ed25519 => unsafe {
                ED25519Authority::load_unchecked(authority_data)?
            },
            AuthorityType::Ed25519Session => unsafe {
                Ed25519SessionAuthority::load_unchecked(authority_data)?
            },
            AuthorityType::Secp256k1 => unsafe {
                Secp256k1Authority::load_unchecked(authority_data)?
            },
            AuthorityType::Secp256k1Session => unsafe {
                Secp256k1SessionAuthority::load_unchecked(authority_data)?
            },
            AuthorityType::Secp256r1 => unsafe {
                Secp256r1Authority::load_unchecked(authority_data)?
            },
            AuthorityType::Secp256r1Session => unsafe {
                Secp256r1SessionAuthority::load_unchecked(authority_data)?
            },
            AuthorityType::ProgramExec => unsafe {
                ProgramExecAuthority::load_unchecked(authority_data)?
            },
            AuthorityType::ProgramExecSession => unsafe {
                ProgramExecSessionAuthority::load_unchecked(authority_data)?
            },
            _ => return Err(ProgramError::InvalidAccountData),
        };
        
        Ok(authority)
    }
    
    /// Load mutable authority as AuthorityInfo trait object from account data
    pub fn load_authority_info_mut<'a>(
        &self,
        account_data: &'a mut [u8],
    ) -> Result<&'a mut dyn AuthorityInfo, ProgramError> {
        let authority_data = &mut account_data[Self::LEN..];
        let auth_type = self.authority_type()?;
        
        let authority: &mut dyn AuthorityInfo = match auth_type {
            AuthorityType::Ed25519 => unsafe {
                ED25519Authority::load_mut_unchecked(authority_data)?
            },
            AuthorityType::Ed25519Session => unsafe {
                Ed25519SessionAuthority::load_mut_unchecked(authority_data)?
            },
            AuthorityType::Secp256k1 => unsafe {
                Secp256k1Authority::load_mut_unchecked(authority_data)?
            },
            AuthorityType::Secp256k1Session => unsafe {
                Secp256k1SessionAuthority::load_mut_unchecked(authority_data)?
            },
            AuthorityType::Secp256r1 => unsafe {
                Secp256r1Authority::load_mut_unchecked(authority_data)?
            },
            AuthorityType::Secp256r1Session => unsafe {
                Secp256r1SessionAuthority::load_mut_unchecked(authority_data)?
            },
            AuthorityType::ProgramExec => unsafe {
                ProgramExecAuthority::load_mut_unchecked(authority_data)?
            },
            AuthorityType::ProgramExecSession => unsafe {
                ProgramExecSessionAuthority::load_mut_unchecked(authority_data)?
            },
            _ => return Err(ProgramError::InvalidAccountData),
        };
        
        Ok(authority)
    }
}

impl Transmutable for WalletAuthority {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for WalletAuthority {}

impl IntoBytes for WalletAuthority {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        let bytes = unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN)
        };
        Ok(bytes)
    }
}
