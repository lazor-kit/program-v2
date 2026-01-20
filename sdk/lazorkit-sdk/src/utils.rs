use crate::core::connection::SolConnection;
use crate::error::{LazorSdkError, Result};
use crate::types::{RoleInfo, WalletInfo};
use lazorkit_state::authority::AuthorityType;
use lazorkit_state::{LazorKitWallet, Position, Transmutable};
use solana_sdk::pubkey::Pubkey;

//=============================================================================
// PDA Derivation Helpers
//=============================================================================

/// Derive the Config PDA from program ID and wallet ID
pub fn derive_config_pda(program_id: &Pubkey, wallet_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"lazorkit", wallet_id], program_id)
}

/// Derive the Vault PDA from program ID and config PDA
pub fn derive_vault_pda(program_id: &Pubkey, config_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        program_id,
    )
}

//=============================================================================
// Account Fetching & Parsing
//=============================================================================

/// Fetch wallet account data from the blockchain
pub async fn fetch_wallet_account(
    connection: &impl SolConnection,
    config_pda: &Pubkey,
) -> Result<Vec<u8>> {
    let account = connection
        .get_account(config_pda)
        .await
        .map_err(|e| LazorSdkError::Connection(e.to_string()))?
        .ok_or_else(|| LazorSdkError::AccountNotFound(*config_pda))?;

    Ok(account.data)
}

/// Parse wallet header from account data
pub fn parse_wallet_header(data: &[u8]) -> Result<LazorKitWallet> {
    if data.len() < LazorKitWallet::LEN {
        return Err(LazorSdkError::InvalidAccountData(
            "Account data too small for wallet header".to_string(),
        ));
    }

    let wallet_ref = unsafe {
        LazorKitWallet::load_unchecked(&data[..LazorKitWallet::LEN]).map_err(|e| {
            LazorSdkError::InvalidAccountData(format!("Failed to parse header: {:?}", e))
        })?
    };

    // Copy the wallet data to return owned value
    Ok(*wallet_ref)
}

/// Parse all roles from wallet account data
pub fn parse_roles(data: &[u8]) -> Result<Vec<RoleInfo>> {
    let wallet = parse_wallet_header(data)?;
    let mut roles = Vec::new();

    let role_buffer = &data[LazorKitWallet::LEN..];
    let mut cursor = 0;

    for _ in 0..wallet.role_count {
        if cursor + Position::LEN > role_buffer.len() {
            return Err(LazorSdkError::InvalidAccountData(
                "Insufficient data for position header".to_string(),
            ));
        }

        let position = unsafe {
            Position::load_unchecked(&role_buffer[cursor..cursor + Position::LEN]).map_err(|e| {
                LazorSdkError::InvalidAccountData(format!("Failed to parse position: {:?}", e))
            })?
        };

        let auth_start = cursor + Position::LEN;
        let auth_end = auth_start + position.authority_length as usize;

        if auth_end > role_buffer.len() {
            return Err(LazorSdkError::InvalidAccountData(
                "Insufficient data for authority".to_string(),
            ));
        }

        let auth_data = &role_buffer[auth_start..auth_end];
        let role_info = parse_role_info(*position, auth_data)?;
        roles.push(role_info);

        cursor = position.boundary as usize - LazorKitWallet::LEN;
    }

    Ok(roles)
}

/// Parse a single role from position and authority data
fn parse_role_info(position: Position, auth_data: &[u8]) -> Result<RoleInfo> {
    let auth_type = AuthorityType::try_from(position.authority_type).map_err(|_| {
        LazorSdkError::InvalidAccountData(format!(
            "Invalid authority type: {}",
            position.authority_type
        ))
    })?;

    let (
        ed25519_pubkey,
        secp256r1_pubkey,
        has_session_support,
        session_key,
        max_session_length,
        max_session_age,
        current_session_expiration,
        signature_odometer,
    ) = match auth_type {
        AuthorityType::Ed25519 => {
            // Layout: [0..32] public_key
            if auth_data.len() >= 32 {
                let mut pubkey = [0u8; 32];
                pubkey.copy_from_slice(&auth_data[..32]);
                (Some(pubkey), None, false, None, None, None, None, None)
            } else {
                (None, None, false, None, None, None, None, None)
            }
        },
        AuthorityType::Ed25519Session => {
            // Layout: [0..32] master_key, [32..64] session_key,
            //         [64..72] max_session_length, [72..80] current_session_expiration
            if auth_data.len() >= 80 {
                let mut master_key = [0u8; 32];
                master_key.copy_from_slice(&auth_data[..32]);

                let mut sess_key = [0u8; 32];
                sess_key.copy_from_slice(&auth_data[32..64]);

                let max_len = u64::from_le_bytes(auth_data[64..72].try_into().unwrap());
                let exp = u64::from_le_bytes(auth_data[72..80].try_into().unwrap());

                (
                    Some(master_key),
                    None,
                    true,
                    Some(sess_key),
                    Some(max_len),
                    None,
                    Some(exp),
                    None,
                )
            } else {
                (None, None, true, None, None, None, None, None)
            }
        },
        AuthorityType::Secp256r1 => {
            // Layout: [0..33] compressed_pubkey, [33..36] _padding, [36..40] signature_odometer
            if auth_data.len() >= 40 {
                let mut pubkey = [0u8; 33];
                pubkey.copy_from_slice(&auth_data[..33]);

                // Parse signature_odometer at [36..40] (skip padding [33..36])
                let odometer = u32::from_le_bytes(auth_data[36..40].try_into().unwrap());

                (
                    None,
                    Some(pubkey),
                    false,
                    None,
                    None,
                    None,
                    None,
                    Some(odometer),
                )
            } else {
                (None, None, false, None, None, None, None, None)
            }
        },
        AuthorityType::Secp256r1Session => {
            // Layout: [0..33] master_compressed_pubkey, [33..36] _padding,
            //         [36..40] signature_odometer, [40..72] session_key,
            //         [72..80] max_session_age, [80..88] current_session_expiration
            if auth_data.len() >= 88 {
                // Parse master key [0..33]
                let mut master_key = [0u8; 33];
                master_key.copy_from_slice(&auth_data[..33]);

                // Parse signature_odometer [36..40] (skip padding [33..36])
                let odometer = u32::from_le_bytes(auth_data[36..40].try_into().unwrap());

                // Parse session_key [40..72]
                let mut sess_key = [0u8; 32];
                sess_key.copy_from_slice(&auth_data[40..72]);

                // Parse max_session_age [72..80]
                let max_age = u64::from_le_bytes(auth_data[72..80].try_into().unwrap());

                // Parse expiration [80..88]
                let exp = u64::from_le_bytes(auth_data[80..88].try_into().unwrap());

                (
                    None,
                    Some(master_key),
                    true,
                    Some(sess_key),
                    None,
                    Some(max_age),
                    Some(exp),
                    Some(odometer),
                )
            } else {
                (None, None, true, None, None, None, None, None)
            }
        },
        _ => {
            return Err(LazorSdkError::InvalidAccountData(format!(
                "Unsupported authority type: {:?}",
                auth_type
            )))
        },
    };

    Ok(RoleInfo {
        id: position.id,
        authority_type: auth_type,
        ed25519_pubkey,
        secp256r1_pubkey,
        has_session_support,
        session_key,
        max_session_length,
        max_session_age,
        current_session_expiration,
        signature_odometer,
    })
}

/// Fetch and parse complete wallet information
pub async fn fetch_wallet_info(
    connection: &impl SolConnection,
    config_pda: &Pubkey,
) -> Result<WalletInfo> {
    let data = fetch_wallet_account(connection, config_pda).await?;
    let wallet = parse_wallet_header(&data)?;
    let roles = parse_roles(&data)?;

    Ok(WalletInfo {
        role_count: wallet.role_count as u32,
        role_counter: wallet.role_counter,
        vault_bump: wallet.wallet_bump,
        roles,
    })
}

/// Find a specific role by ID
pub fn find_role(roles: &[RoleInfo], role_id: u32) -> Option<&RoleInfo> {
    roles.iter().find(|r| r.id == role_id)
}

//=============================================================================
// Secp256r1 Signature Helpers
//=============================================================================

/// Build Secp256r1 authorization payload for standard authentication
///
/// # Arguments
/// * `authority_slot` - Slot number when signature was created
/// * `counter` - Signature counter (must be current odometer + 1)
/// * `instruction_account_index` - Index of Instructions sysvar in accounts
/// * `webauthn_data` - Optional WebAuthn-specific data
///
/// # Returns
/// Properly formatted authorization payload for Secp256r1 authentication
///
/// # Layout
/// ```
/// [0..8]   authority_slot: u64
/// [8..12]  counter: u32
/// [12]     instruction_account_index: u8
/// [13..17] reserved: [u8; 4]
/// [17..]   optional WebAuthn data
/// ```
pub fn build_secp256r1_auth_payload(
    authority_slot: u64,
    counter: u32,
    instruction_account_index: u8,
    webauthn_data: Option<&[u8]>,
) -> Vec<u8> {
    let mut payload = Vec::new();

    // Core fields
    payload.extend_from_slice(&authority_slot.to_le_bytes());
    payload.extend_from_slice(&counter.to_le_bytes());
    payload.push(instruction_account_index);

    // Reserved bytes
    payload.extend_from_slice(&[0u8; 4]);

    // Optional WebAuthn data
    if let Some(data) = webauthn_data {
        payload.extend_from_slice(data);
    }

    payload
}
