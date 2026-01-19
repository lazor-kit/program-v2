//! Secp256r1.rs AccountsPayload struct definition
//! This is a shared utility struct used by secp256r1 for message hashing

use crate::{IntoBytes, Transmutable};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

/// Represents the account payload structure for signature verification.
/// This structure is used to encode account information that gets included
/// in the message being signed.
/// Represents account information in a format suitable for payload
/// construction.
#[repr(C, align(8))]
#[derive(Copy, Clone, no_padding::NoPadding)]
pub struct AccountsPayload {
    /// The account's public key
    pub pubkey: Pubkey,
    /// Whether the account is writable
    pub is_writable: bool,
    /// Whether the account is a signer
    pub is_signer: bool,
    _padding: [u8; 6],
}

impl AccountsPayload {
    /// Creates a new AccountsPayload.
    ///
    /// # Arguments
    /// * `pubkey` - The account's public key
    /// * `is_writable` - Whether the account is writable
    /// * `is_signer` - Whether the account is a signer
    pub fn new(pubkey: Pubkey, is_writable: bool, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_writable,
            is_signer,
            _padding: [0u8; 6],
        }
    }
}

impl Transmutable for AccountsPayload {
    const LEN: usize = core::mem::size_of::<AccountsPayload>();
}

impl IntoBytes for AccountsPayload {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        let bytes =
            unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) };
        Ok(bytes)
    }
}

impl From<&AccountInfo> for AccountsPayload {
    fn from(info: &AccountInfo) -> Self {
        Self::new(*info.key(), info.is_writable(), info.is_signer())
    }
}

pub fn hex_encode(input: &[u8], output: &mut [u8]) {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    for (i, &byte) in input.iter().enumerate() {
        output[i * 2] = HEX_CHARS[(byte >> 4) as usize];
        output[i * 2 + 1] = HEX_CHARS[(byte & 0x0F) as usize];
    }
}
