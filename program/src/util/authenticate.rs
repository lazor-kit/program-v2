//! Authority authentication utilities

use lazorkit_v2_state::{
    authority::{Authority, AuthorityInfo, AuthorityType},
    wallet_account::AuthorityData,
    Transmutable,
};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

/// Authenticate authority for an operation (verify signature only)
///
/// **Key Difference:**
/// - In Lazorkit V2: Authentication (verify signature) is done here in lazorkit-v2
/// - Authorization/permission check is done by plugins via CPI
/// - Lazorkit V2 does NOT know which authority has what permissions
/// - Only plugins know and enforce permission rules
///
/// This function ONLY verifies the signature to prove the authority is authentic.
/// Permission checking is delegated to plugins via CPI calls.
pub fn authenticate_authority(
    authority_data: &AuthorityData,
    accounts: &[AccountInfo],
    authority_payload: Option<&[u8]>,
    data_payload: Option<&[u8]>,
) -> ProgramResult {
    // If no authority_payload provided, skip authentication (optional in Pure External)
    // Plugins can handle authentication if needed
    let authority_payload = match authority_payload {
        Some(payload) => payload,
        None => return Ok(()), // Skip authentication if not provided
    };

    // If authority_payload is empty, skip authentication
    if authority_payload.is_empty() {
        return Ok(());
    }

    let data_payload = data_payload.unwrap_or(&[]);

    // Get current slot
    let clock = Clock::get()?;
    let slot = clock.slot;

    // Parse authority type
    let authority_type =
        AuthorityType::try_from(authority_data.position.authority_type).map_err(|_| {
            // Return more specific error
            ProgramError::InvalidInstructionData
        })?;

    // Check if authority is session-based
    // Session-based authority types: Ed25519Session (2), Secp256k1Session (4), Secp256r1Session (6), ProgramExecSession (8)
    let is_session_based = matches!(
        authority_data.position.authority_type,
        2 | 4 | 6 | 8 // Session variants
    );

    // Authenticate based on authority type
    // Check session_based() first, then call authenticate_session() or authenticate()
    match authority_type {
        AuthorityType::Ed25519 | AuthorityType::Ed25519Session => {
            if is_session_based {
                use lazorkit_v2_state::authority::ed25519::Ed25519SessionAuthority;
                // Parse session authority from authority_data
                if authority_data.authority_data.len() < 80 {
                    return Err(ProgramError::InvalidAccountData);
                }
                // Create mutable copy for authentication using ptr::read
                let mut authority_bytes = [0u8; 80];
                authority_bytes.copy_from_slice(&authority_data.authority_data[..80]);
                let authority_ref =
                    unsafe { Ed25519SessionAuthority::load_unchecked(&authority_bytes)? };
                // Copy using ptr::read (safe for Copy types, but we need it for non-Copy)
                let mut authority: Ed25519SessionAuthority =
                    unsafe { core::ptr::read(authority_ref as *const Ed25519SessionAuthority) };
                // Call authenticate_session() if session_based()
                authority.authenticate_session(accounts, authority_payload, data_payload, slot)?;
            } else {
                use lazorkit_v2_state::authority::ed25519::ED25519Authority;
                // ED25519Authority requires exactly 32 bytes (public_key)
                if authority_data.authority_data.len() != 32 {
                    return Err(ProgramError::InvalidAccountData);
                }
                let mut authority =
                    ED25519Authority::from_create_bytes(&authority_data.authority_data)?;
                // Call authenticate() if not session_based()
                authority.authenticate(accounts, authority_payload, data_payload, slot)?;
            }
        },
        AuthorityType::Secp256k1 | AuthorityType::Secp256k1Session => {
            if is_session_based {
                use lazorkit_v2_state::authority::secp256k1::Secp256k1SessionAuthority;
                if authority_data.authority_data.len() < 88 {
                    return Err(ProgramError::InvalidAccountData);
                }
                // Create mutable copy for authentication using ptr::read
                let mut authority_bytes = [0u8; 88];
                authority_bytes.copy_from_slice(&authority_data.authority_data[..88]);
                let authority_ref =
                    unsafe { Secp256k1SessionAuthority::load_unchecked(&authority_bytes)? };
                // Copy using ptr::read
                let mut authority: Secp256k1SessionAuthority =
                    unsafe { core::ptr::read(authority_ref as *const Secp256k1SessionAuthority) };
                // Call authenticate_session() if session_based()
                authority.authenticate_session(accounts, authority_payload, data_payload, slot)?;
            } else {
                use lazorkit_v2_state::authority::secp256k1::Secp256k1Authority;
                // Secp256k1Authority requires public_key (33 bytes)
                if authority_data.authority_data.len() < 33 {
                    return Err(ProgramError::InvalidAccountData);
                }
                let mut public_key = [0u8; 33];
                public_key.copy_from_slice(&authority_data.authority_data[..33]);
                let mut authority = Secp256k1Authority::new(public_key);
                // Call authenticate() if not session_based()
                authority.authenticate(accounts, authority_payload, data_payload, slot)?;
            }
        },
        AuthorityType::Secp256r1 | AuthorityType::Secp256r1Session => {
            if is_session_based {
                use lazorkit_v2_state::authority::secp256r1::Secp256r1SessionAuthority;
                if authority_data.authority_data.len() < 88 {
                    return Err(ProgramError::InvalidAccountData);
                }
                // Create mutable copy for authentication using ptr::read
                let mut authority_bytes = [0u8; 88];
                authority_bytes.copy_from_slice(&authority_data.authority_data[..88]);
                let authority_ref =
                    unsafe { Secp256r1SessionAuthority::load_unchecked(&authority_bytes)? };
                // Copy using ptr::read
                let mut authority: Secp256r1SessionAuthority =
                    unsafe { core::ptr::read(authority_ref as *const Secp256r1SessionAuthority) };
                // Call authenticate_session() if session_based()
                authority.authenticate_session(accounts, authority_payload, data_payload, slot)?;
            } else {
                use lazorkit_v2_state::authority::secp256r1::Secp256r1Authority;
                // Secp256r1Authority requires public_key (33 bytes)
                if authority_data.authority_data.len() < 33 {
                    return Err(ProgramError::InvalidAccountData);
                }
                let mut public_key = [0u8; 33];
                public_key.copy_from_slice(&authority_data.authority_data[..33]);
                let mut authority = Secp256r1Authority::new(public_key);
                // Call authenticate() if not session_based()
                authority.authenticate(accounts, authority_payload, data_payload, slot)?;
            }
        },
        AuthorityType::ProgramExec | AuthorityType::ProgramExecSession => {
            if is_session_based {
                use lazorkit_v2_state::authority::programexec::session::ProgramExecSessionAuthority;
                if authority_data.authority_data.len() < 80 {
                    return Err(ProgramError::InvalidAccountData);
                }
                // Create mutable copy for authentication using ptr::read
                let mut authority_bytes = [0u8; 80];
                authority_bytes.copy_from_slice(&authority_data.authority_data[..80]);
                let authority_ref =
                    unsafe { ProgramExecSessionAuthority::load_unchecked(&authority_bytes)? };
                // Copy using ptr::read
                let mut authority: ProgramExecSessionAuthority =
                    unsafe { core::ptr::read(authority_ref as *const ProgramExecSessionAuthority) };
                // Call authenticate_session() if session_based()
                authority.authenticate_session(accounts, authority_payload, data_payload, slot)?;
            } else {
                use lazorkit_v2_state::authority::programexec::ProgramExecAuthority;
                // ProgramExecAuthority requires program_id (32) + instruction_prefix_len (1) = 33 bytes
                if authority_data.authority_data.len() < 33 {
                    return Err(ProgramError::InvalidAccountData);
                }
                let mut program_id_bytes = [0u8; 32];
                program_id_bytes.copy_from_slice(&authority_data.authority_data[..32]);
                let instruction_prefix_len = authority_data.authority_data[32];
                let mut authority =
                    ProgramExecAuthority::new(program_id_bytes, instruction_prefix_len);
                // Call authenticate() if not session_based()
                authority.authenticate(accounts, authority_payload, data_payload, slot)?;
            }
        },
        AuthorityType::None => {
            return Err(ProgramError::InvalidAccountData);
        },
    }

    Ok(())
}
