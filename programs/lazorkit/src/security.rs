use anchor_lang::prelude::*;

/// LazorKit security constants and validation utilities
/// 
/// Contains security-related constants and validation functions used throughout
/// the LazorKit program to ensure safe operation and prevent various attack
/// vectors including DoS, overflow, and unauthorized access.

// === Size Limits ===
/// Maximum allowed size for credential ID to prevent DoS attacks
pub const MAX_CREDENTIAL_ID_SIZE: usize = 256;

/// Maximum allowed size for policy data to prevent excessive memory usage
pub const MAX_POLICY_DATA_SIZE: usize = 1024;

/// Maximum allowed size for CPI data to prevent resource exhaustion
pub const MAX_CPI_DATA_SIZE: usize = 1024;

/// Maximum allowed remaining accounts to prevent account exhaustion
pub const MAX_REMAINING_ACCOUNTS: usize = 32;

// === Financial Limits ===
/// Minimum rent-exempt balance buffer (in lamports) to ensure account viability
pub const MIN_RENT_EXEMPT_BUFFER: u64 = 1_000_000; // 0.001 SOL

// === Time-based Security ===
/// Maximum transaction age in seconds to prevent replay attacks
pub const MAX_TRANSACTION_AGE: i64 = 300; // 5 minutes

/// Maximum allowed session TTL in seconds for deferred execution
pub const MAX_SESSION_TTL_SECONDS: i64 = 30; // 30 seconds

// === Rate Limiting ===
/// Maximum transactions per block to prevent spam
pub const MAX_TRANSACTIONS_PER_BLOCK: u8 = 5;
/// Rate limiting window in blocks
pub const RATE_LIMIT_WINDOW_BLOCKS: u64 = 10;

/// Security validation functions
pub mod validation {
    use super::*;
    use crate::error::LazorKitError;

    /// Validate credential ID size
    pub fn validate_credential_id(credential_id: &[u8]) -> Result<()> {
        require!(
            credential_id.len() <= MAX_CREDENTIAL_ID_SIZE,
            LazorKitError::CredentialIdTooLarge
        );
        require!(!credential_id.is_empty(), LazorKitError::CredentialIdEmpty);
        Ok(())
    }

    /// Validate policy data size
    pub fn validate_policy_data(policy_data: &[u8]) -> Result<()> {
        require!(
            policy_data.len() <= MAX_POLICY_DATA_SIZE,
            LazorKitError::PolicyDataTooLarge
        );
        Ok(())
    }

    /// Validate CPI data
    pub fn validate_cpi_data(cpi_data: &[u8]) -> Result<()> {
        require!(
            cpi_data.len() <= MAX_CPI_DATA_SIZE,
            LazorKitError::CpiDataTooLarge
        );
        require!(!cpi_data.is_empty(), LazorKitError::CpiDataMissing);
        Ok(())
    }

    /// Validate CPI data when a blob hash may be present. If `has_hash` is true,
    /// inline cpi_data can be empty; otherwise, it must be non-empty.
    pub fn validate_cpi_data_or_hash(cpi_data: &[u8], has_hash: bool) -> Result<()> {
        require!(
            cpi_data.len() <= MAX_CPI_DATA_SIZE,
            LazorKitError::CpiDataTooLarge
        );
        if !has_hash {
            require!(!cpi_data.is_empty(), LazorKitError::CpiDataMissing);
        }
        Ok(())
    }

    /// Validate remaining accounts count
    pub fn validate_remaining_accounts(accounts: &[AccountInfo]) -> Result<()> {
        require!(
            accounts.len() <= MAX_REMAINING_ACCOUNTS,
            LazorKitError::TooManyRemainingAccounts
        );
        Ok(())
    }

    /// Validate lamport amount to prevent overflow
    pub fn validate_lamport_amount(amount: u64) -> Result<()> {
        // Ensure amount doesn't cause overflow in calculations
        require!(
            amount <= u64::MAX / 2,
            LazorKitError::TransferAmountOverflow
        );
        Ok(())
    }

    /// Validate program is executable
    pub fn validate_program_executable(program: &AccountInfo) -> Result<()> {
        require!(program.executable, LazorKitError::ProgramNotExecutable);

        require!(
            program.key() != crate::ID,
            LazorKitError::ReentrancyDetected
        );
        Ok(())
    }

    /// Validate account ownership
    pub fn validate_account_owner(account: &AccountInfo, expected_owner: &Pubkey) -> Result<()> {
        require!(
            account.owner == expected_owner,
            LazorKitError::InvalidAccountOwner
        );
        Ok(())
    }

    /// Validate PDA derivation
    pub fn validate_pda(
        account: &AccountInfo,
        seeds: &[&[u8]],
        program_id: &Pubkey,
        bump: u8,
    ) -> Result<()> {
        let (expected_key, expected_bump) = Pubkey::find_program_address(seeds, program_id);
        require!(
            account.key() == expected_key,
            LazorKitError::InvalidPDADerivation
        );
        require!(bump == expected_bump, LazorKitError::InvalidBumpSeed);
        Ok(())
    }

    /// Validate timestamp is within acceptable range
    pub fn validate_timestamp(timestamp: i64, current_time: i64) -> Result<()> {
        let age = current_time.saturating_sub(timestamp);
        require!(
            age >= 0 && age <= MAX_TRANSACTION_AGE,
            LazorKitError::TransactionTooOld
        );
        Ok(())
    }
}
