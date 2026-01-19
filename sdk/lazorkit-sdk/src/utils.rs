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
        max_session_length,
        current_session_expiration,
    ) = match auth_type {
        AuthorityType::Ed25519 => {
            if auth_data.len() >= 32 {
                let mut pubkey = [0u8; 32];
                pubkey.copy_from_slice(&auth_data[..32]);
                (Some(pubkey), None, false, None, None)
            } else {
                (None, None, false, None, None)
            }
        },
        AuthorityType::Ed25519Session => {
            if auth_data.len() >= 80 {
                let mut master_key = [0u8; 32];
                master_key.copy_from_slice(&auth_data[..32]);

                let max_len = u64::from_le_bytes(auth_data[64..72].try_into().unwrap());
                let exp = u64::from_le_bytes(auth_data[72..80].try_into().unwrap());

                (Some(master_key), None, true, Some(max_len), Some(exp))
            } else {
                (None, None, true, None, None)
            }
        },
        AuthorityType::Secp256r1 => {
            if auth_data.len() >= 33 {
                let mut pubkey = [0u8; 33];
                pubkey.copy_from_slice(&auth_data[..33]);
                (None, Some(pubkey), false, None, None)
            } else {
                (None, None, false, None, None)
            }
        },
        AuthorityType::Secp256r1Session => {
            if auth_data.len() >= 73 {
                let mut master_key = [0u8; 33];
                master_key.copy_from_slice(&auth_data[..33]);

                let max_len = u64::from_le_bytes(auth_data[65..73].try_into().unwrap());
                let exp = u64::from_le_bytes(auth_data[73..81].try_into().unwrap());

                (None, Some(master_key), true, Some(max_len), Some(exp))
            } else {
                (None, None, true, None, None)
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
        max_session_length,
        current_session_expiration,
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
