//! Builder for constructing and modifying LazorKit wallet accounts.

use crate::{
    authority::{
        ed25519::{Ed25519Authority, Ed25519SessionAuthority},
        programexec::{ProgramExecAuthority, ProgramExecSessionAuthority},
        secp256k1::{Secp256k1Authority, Secp256k1SessionAuthority},
        secp256r1::{Secp256r1Authority, Secp256r1SessionAuthority},
        Authority, AuthorityType,
    },
    policy::count_policies,
    IntoBytes, LazorKitWallet, Position, Transmutable, TransmutableMut,
};
use pinocchio::program_error::ProgramError;

/// Builder for constructing and modifying LazorKit wallet accounts.
pub struct LazorKitBuilder<'a> {
    /// Buffer for role data
    pub role_buffer: &'a mut [u8],
    /// Reference to the LazorKitWallet account being built
    pub wallet: &'a mut LazorKitWallet,
}

impl<'a> LazorKitBuilder<'a> {
    /// Creates a new LazorKitBuilder from account buffer and LazorKitWallet data.
    pub fn create(
        account_buffer: &'a mut [u8],
        wallet: LazorKitWallet,
    ) -> Result<Self, ProgramError> {
        let (wallet_bytes, roles_bytes) = account_buffer.split_at_mut(LazorKitWallet::LEN);
        let bytes = wallet.into_bytes()?;
        wallet_bytes[0..].copy_from_slice(bytes);
        let builder = Self {
            role_buffer: roles_bytes,
            wallet: unsafe { LazorKitWallet::load_mut_unchecked(wallet_bytes)? },
        };
        Ok(builder)
    }

    /// Creates a new LazorKitBuilder from raw account bytes.
    pub fn new_from_bytes(account_buffer: &'a mut [u8]) -> Result<Self, ProgramError> {
        let (wallet_bytes, roles_bytes) = account_buffer.split_at_mut(LazorKitWallet::LEN);
        let wallet = unsafe { LazorKitWallet::load_mut_unchecked(wallet_bytes)? };
        let builder = Self {
            role_buffer: roles_bytes,
            wallet,
        };
        Ok(builder)
    }

    /// Adds a new role to the LazorKit wallet account.
    ///
    /// # Arguments
    /// * `authority_type` - The type of authority for this role
    /// * `authority_data` - Raw bytes containing the authority data
    /// * `actions_data` - Raw bytes containing the actions data (can be empty for no plugins)
    ///
    /// # Returns
    /// * `Result<(), ProgramError>` - Success or error status
    pub fn add_role(
        &mut self,
        authority_type: AuthorityType,
        authority_data: &[u8],
        _policies_data: &'a [u8], // For future policy support
    ) -> Result<(), ProgramError> {
        // Find cursor position (end of last role or start if no roles)
        let mut cursor = 0;
        for _i in 0..self.wallet.role_count {
            let position = unsafe {
                Position::load_unchecked(&self.role_buffer[cursor..cursor + Position::LEN])?
            };
            cursor = (position.boundary as usize)
                .checked_sub(LazorKitWallet::LEN)
                .ok_or(ProgramError::InvalidAccountData)?;
        }

        let auth_offset = cursor + Position::LEN;

        // Set authority data based on type
        let authority_length = match authority_type {
            AuthorityType::Ed25519 => {
                Ed25519Authority::set_into_bytes(
                    authority_data,
                    &mut self.role_buffer[auth_offset..auth_offset + Ed25519Authority::LEN],
                )?;
                Ed25519Authority::LEN
            },
            AuthorityType::Ed25519Session => {
                Ed25519SessionAuthority::set_into_bytes(
                    authority_data,
                    &mut self.role_buffer[auth_offset..auth_offset + Ed25519SessionAuthority::LEN],
                )?;
                Ed25519SessionAuthority::LEN
            },
            AuthorityType::Secp256k1 => {
                Secp256k1Authority::set_into_bytes(
                    authority_data,
                    &mut self.role_buffer[auth_offset..auth_offset + Secp256k1Authority::LEN],
                )?;
                Secp256k1Authority::LEN
            },
            AuthorityType::Secp256k1Session => {
                Secp256k1SessionAuthority::set_into_bytes(
                    authority_data,
                    &mut self.role_buffer
                        [auth_offset..auth_offset + Secp256k1SessionAuthority::LEN],
                )?;
                Secp256k1SessionAuthority::LEN
            },
            AuthorityType::Secp256r1 => {
                Secp256r1Authority::set_into_bytes(
                    authority_data,
                    &mut self.role_buffer[auth_offset..auth_offset + Secp256r1Authority::LEN],
                )?;
                Secp256r1Authority::LEN
            },
            AuthorityType::Secp256r1Session => {
                Secp256r1SessionAuthority::set_into_bytes(
                    authority_data,
                    &mut self.role_buffer
                        [auth_offset..auth_offset + Secp256r1SessionAuthority::LEN],
                )?;
                Secp256r1SessionAuthority::LEN
            },
            AuthorityType::ProgramExec => {
                ProgramExecAuthority::set_into_bytes(
                    authority_data,
                    &mut self.role_buffer[auth_offset..auth_offset + ProgramExecAuthority::LEN],
                )?;
                ProgramExecAuthority::LEN
            },
            AuthorityType::ProgramExecSession => {
                ProgramExecSessionAuthority::set_into_bytes(
                    authority_data,
                    &mut self.role_buffer
                        [auth_offset..auth_offset + ProgramExecSessionAuthority::LEN],
                )?;
                ProgramExecSessionAuthority::LEN
            },
            _ => return Err(ProgramError::InvalidInstructionData),
        };

        // Calculate policies offset and copy policy data to buffer
        // NOTE: This is opaque storage - no validation at builder level
        // TODO: Policy validation happens via CPI during Execute flow
        //       Core will CPI to each policy's verify() to validate state
        let policies_offset = auth_offset + authority_length;
        if !_policies_data.is_empty() {
            self.role_buffer[policies_offset..policies_offset + _policies_data.len()]
                .copy_from_slice(_policies_data);
        }

        // Calculate boundary: Position + Authority + Policies
        let size = authority_length + _policies_data.len();
        let relative_boundary = cursor + Position::LEN + size;
        let absolute_boundary = relative_boundary + LazorKitWallet::LEN;

        // Write Position header
        let position = unsafe {
            Position::load_mut_unchecked(&mut self.role_buffer[cursor..cursor + Position::LEN])?
        };
        position.authority_type = authority_type as u16;
        position.authority_length = authority_length as u16;
        position.num_policies = if _policies_data.is_empty() {
            0
        } else {
            count_policies(_policies_data)?
        };
        position.padding = 0;
        position.id = self.wallet.role_counter;
        position.boundary = absolute_boundary as u32;

        // Update wallet counters
        self.wallet.role_count += 1;
        self.wallet.role_counter += 1;

        Ok(())
    }
}
