use anchor_lang::prelude::*;

/// LazorKit security constants and validation utilities
///
/// Contains security-related constants and validation functions used throughout
/// the LazorKit program to ensure safe operation and prevent various attack
/// vectors including DoS, overflow, and unauthorized access.

// === Size Limits ===
/// Maximum allowed size for credential ID to prevent DoS attacks
/// Rationale: WebAuthn credential IDs are typically 16-64 bytes, 256 provides safety margin
pub const MAX_CREDENTIAL_ID_SIZE: usize = 256;

/// Maximum allowed size for policy data to prevent excessive memory usage
/// Rationale: Policy instructions should be concise, 1KB allows for complex policies while preventing DoS
pub const MAX_POLICY_DATA_SIZE: usize = 1024;

/// Maximum allowed size for CPI data to prevent resource exhaustion
/// Rationale: CPI instructions should be reasonable size, 1KB prevents memory exhaustion attacks
pub const MAX_CPI_DATA_SIZE: usize = 1024;

/// Maximum allowed remaining accounts to prevent account exhaustion
/// Rationale: Solana transaction limit is ~64 accounts, 32 provides safety margin
pub const MAX_REMAINING_ACCOUNTS: usize = 32;

// === Financial Limits ===
/// Minimum rent-exempt balance buffer (in lamports) to ensure account viability
/// Rationale: Ensures accounts remain rent-exempt even with small SOL transfers
pub const MIN_RENT_EXEMPT_BUFFER: u64 = 1_000_000; // 0.001 SOL

// === Time-based Security ===
/// Maximum allowed session TTL in seconds for deferred execution
/// Rationale: 30 seconds prevents long-lived sessions that could be exploited
pub const MAX_SESSION_TTL_SECONDS: i64 = 30; // 30 seconds

/// Standard timestamp validation window (past tolerance in seconds)
/// Rationale: 30 seconds provides reasonable window while preventing old transaction replay
pub const TIMESTAMP_PAST_TOLERANCE: i64 = 30; // 30 seconds

/// Standard timestamp validation window (future tolerance in seconds)
/// Rationale: 30 seconds allows for reasonable clock skew while preventing future-dated attacks
pub const TIMESTAMP_FUTURE_TOLERANCE: i64 = 30; // 30 seconds

/// Security validation functions
pub mod validation {
    use super::*;
    use crate::{error::LazorKitError, ID};

    pub fn validate_passkey_format(
        passkey_public_key: &[u8; crate::constants::PASSKEY_PUBLIC_KEY_SIZE],
    ) -> Result<()> {
        require!(
            passkey_public_key[0] == crate::constants::SECP256R1_COMPRESSED_PUBKEY_PREFIX_EVEN
                || passkey_public_key[0]
                    == crate::constants::SECP256R1_COMPRESSED_PUBKEY_PREFIX_ODD,
            LazorKitError::InvalidPasskeyFormat
        );
        Ok(())
    }

    pub fn validate_policy_data(policy_data: &[u8], policy_data_size: u16) -> Result<()> {
        require!(
            policy_data_size == policy_data.len() as u16,
            LazorKitError::InvalidPolicyDataSize
        );
        require!(
            policy_data.len() <= MAX_POLICY_DATA_SIZE,
            LazorKitError::PolicyDataTooLarge
        );
        require!(
            !policy_data.is_empty() && policy_data.len() >= 8,
            LazorKitError::InvalidInstructionData
        );
        Ok(())
    }

    pub fn validate_credential_hash(credential_hash: &[u8; 32]) -> Result<()> {
        let is_all_zeros = credential_hash.iter().all(|&b| b == 0);
        require!(!is_all_zeros, LazorKitError::CredentialIdEmpty);
        Ok(())
    }

    pub fn validate_program_executable(program: &AccountInfo) -> Result<()> {
        require!(program.executable, LazorKitError::ProgramNotExecutable);
        require!(program.key() != ID, LazorKitError::ReentrancyDetected);
        Ok(())
    }

    pub fn validate_no_self_reentrancy(program: &AccountInfo) -> Result<()> {
        require!(program.key() != ID, LazorKitError::ReentrancyDetected);
        Ok(())
    }

    pub fn validate_no_reentrancy(remaining_accounts: &[AccountInfo]) -> Result<()> {
        for account in remaining_accounts {
            if account.executable && account.key() == ID {
                return Err(LazorKitError::ReentrancyDetected.into());
            }
        }
        Ok(())
    }

    pub fn validate_instruction_timestamp(timestamp: i64) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(
            timestamp >= now - TIMESTAMP_PAST_TOLERANCE
                && timestamp <= now + TIMESTAMP_FUTURE_TOLERANCE,
            LazorKitError::TransactionTooOld
        );
        Ok(())
    }

    pub fn safe_increment_nonce(current_nonce: u64) -> u64 {
        current_nonce.wrapping_add(1)
    }

    pub fn validate_webauthn_args(
        passkey_public_key: &[u8; crate::constants::PASSKEY_PUBLIC_KEY_SIZE],
        signature: &[u8],
        client_data_json_raw: &[u8],
        authenticator_data_raw: &[u8],
        verify_instruction_index: u8,
    ) -> Result<()> {
        validate_passkey_format(passkey_public_key)?;
        require!(signature.len() == 64, LazorKitError::InvalidSignature);
        require!(
            !client_data_json_raw.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        require!(
            !authenticator_data_raw.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        require!(
            verify_instruction_index <= crate::constants::MAX_VERIFY_INSTRUCTION_INDEX,
            LazorKitError::InvalidInstructionData
        );
        Ok(())
    }
}
