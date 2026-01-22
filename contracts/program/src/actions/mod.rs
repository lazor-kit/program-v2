pub mod add_authority;
pub mod create_session;
pub mod create_wallet;
pub mod execute;
pub mod remove_authority;
pub mod transfer_ownership;
pub mod update_authority;

pub use add_authority::*;
pub use create_session::*;
pub use create_wallet::*;
pub use execute::*;
pub use remove_authority::*;
pub use transfer_ownership::*;
pub use update_authority::*;

use crate::error::LazorKitError;
use lazorkit_state::authority::AuthorityInfo;
use lazorkit_state::{read_position, LazorKitWallet, Position, Transmutable, TransmutableMut};
use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, sysvars::Sysvar,
    ProgramResult,
};

/// Helper to scan for a specific role in the wallet registry.
pub fn find_role(config_data: &[u8], role_id: u32) -> Result<(Position, usize), ProgramError> {
    let mut current_cursor = LazorKitWallet::LEN;
    let wallet = unsafe { LazorKitWallet::load_unchecked(&config_data[..LazorKitWallet::LEN]) }
        .map_err(|_| ProgramError::InvalidAccountData)?;

    // Validate discriminator
    if !wallet.is_valid() {
        return Err(ProgramError::InvalidAccountData);
    }

    let mut remaining = wallet.role_count;

    while remaining > 0 {
        if current_cursor + Position::LEN > config_data.len() {
            break;
        }
        let pos_ref = read_position(&config_data[current_cursor..])?;
        if pos_ref.id == role_id {
            return Ok((*pos_ref, current_cursor));
        }
        current_cursor = pos_ref.boundary as usize;
        remaining -= 1;
    }
    Err(LazorKitError::AuthorityNotFound.into())
}

pub fn authenticate_role(
    config_account: &AccountInfo,
    acting_role_id: u32,
    accounts: &[AccountInfo],
    authorization_data: &[u8],
    data_payload: &[u8],
) -> ProgramResult {
    let mut config_data = config_account.try_borrow_mut_data()?;

    // find_role already validates discriminator
    let (pos, role_abs_offset) = find_role(&config_data, acting_role_id)?;

    let auth_start = role_abs_offset + Position::LEN;
    let auth_end = auth_start + pos.authority_length as usize;
    if auth_end > config_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }

    let auth_type_enum = lazorkit_state::AuthorityType::try_from(pos.authority_type)?;
    let roles_data = &mut config_data[auth_start..auth_end];

    match auth_type_enum {
        lazorkit_state::AuthorityType::Ed25519 => {
            let clock = pinocchio::sysvars::clock::Clock::get()?;
            let auth = unsafe { lazorkit_state::Ed25519Authority::load_mut_unchecked(roles_data)? };
            auth.authenticate(accounts, authorization_data, data_payload, clock.slot)?;
        },
        lazorkit_state::AuthorityType::Ed25519Session => {
            let clock = pinocchio::sysvars::clock::Clock::get()?;
            let auth =
                unsafe { lazorkit_state::Ed25519SessionAuthority::load_mut_unchecked(roles_data)? };
            auth.authenticate(accounts, authorization_data, data_payload, clock.slot)?;
        },
        lazorkit_state::AuthorityType::Secp256r1 => {
            let clock = pinocchio::sysvars::clock::Clock::get()?;
            let auth =
                unsafe { lazorkit_state::Secp256r1Authority::load_mut_unchecked(roles_data)? };
            auth.authenticate(accounts, authorization_data, data_payload, clock.slot)?;
        },
        lazorkit_state::AuthorityType::Secp256r1Session => {
            let clock = pinocchio::sysvars::clock::Clock::get()?;
            let auth = unsafe {
                lazorkit_state::Secp256r1SessionAuthority::load_mut_unchecked(roles_data)?
            };
            auth.authenticate(accounts, authorization_data, data_payload, clock.slot)?;
        },
        _ => {
            msg!(
                "AuthenticateRole: Unsupported authority type {:?}",
                auth_type_enum
            );
            return Err(ProgramError::InvalidInstructionData);
        },
    }

    Ok(())
}

/// Checks if a Position has administrative privileges (Owner or Admin).
///
/// # Arguments
/// * `position` - The Position to check
///
/// # Returns
/// * `true` if role type is Owner (0) or Admin (1)
/// * `false` otherwise
///
/// # RBAC Context
/// Per architecture v3.0.0:
/// - Owner (type 0): Full control including ownership transfer
/// - Admin (type 1): Authority management permissions
/// - Spender (type 2+): Execute-only permissions
#[inline]
pub fn is_admin_or_owner(position: &Position) -> bool {
    position.is_admin_or_owner()
}

/// Verifies that the acting role has administrative privileges.
///
/// # Arguments
/// * `position` - The Position to verify
///
/// # Returns
/// * `Ok(())` if authorized (Owner or Admin)
/// * `Err(LazorKitError::Unauthorized)` if not authorized
///
/// # Use Cases
/// - AddAuthority: Only Owner/Admin can add new roles
/// - RemoveAuthority: Only Owner/Admin can remove roles
/// - UpdateAuthority: Only Owner/Admin can update role data
pub fn require_admin_or_owner(position: &Position) -> ProgramResult {
    if is_admin_or_owner(position) {
        Ok(())
    } else {
        msg!(
            "Permission denied: Only Owner (type 0) or Admin (type 1) can perform this operation. Role ID: {}, Role Type: {}",
            position.id,
            position.role_type
        );
        Err(LazorKitError::Unauthorized.into())
    }
}
