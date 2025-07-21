use anchor_lang::prelude::*;

use crate::error::LazorKitError;

// Security constants and validation utilities

/// Maximum allowed size for credential ID to prevent DoS
pub const MAX_CREDENTIAL_ID_SIZE: usize = 256;

/// Maximum allowed size for rule data
pub const MAX_RULE_DATA_SIZE: usize = 1024;

/// Maximum allowed size for CPI data
pub const MAX_CPI_DATA_SIZE: usize = 1024;

/// Maximum allowed remaining accounts
pub const MAX_REMAINING_ACCOUNTS: usize = 32;

/// Minimum rent-exempt balance buffer (in lamports)
pub const MIN_RENT_EXEMPT_BUFFER: u64 = 1_000_000; // 0.001 SOL

/// Maximum transaction age in seconds
pub const MAX_TRANSACTION_AGE: i64 = 300; // 5 minutes

/// Rate limiting parameters
pub const MAX_TRANSACTIONS_PER_BLOCK: u8 = 5;
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
        require!(
            !credential_id.is_empty(),
            LazorKitError::CredentialIdEmpty
        );
        Ok(())
    }

    /// Validate rule data size
    pub fn validate_rule_data(rule_data: &[u8]) -> Result<()> {
        require!(
            rule_data.len() <= MAX_RULE_DATA_SIZE,
            LazorKitError::RuleDataTooLarge
        );
        Ok(())
    }

    /// Validate CPI data
    pub fn validate_cpi_data(cpi_data: &[u8]) -> Result<()> {
        require!(
            cpi_data.len() <= MAX_CPI_DATA_SIZE,
            LazorKitError::CpiDataTooLarge
        );
        require!(
            !cpi_data.is_empty(),
            LazorKitError::CpiDataMissing
        );
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
        require!(
            program.executable,
            LazorKitError::ProgramNotExecutable
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
        require!(
            bump == expected_bump,
            LazorKitError::InvalidBumpSeed
        );
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

/// Rate limiting implementation
pub struct RateLimiter;

impl RateLimiter {
    /// Check if transaction rate is within limits
    pub fn check_rate_limit(
        transaction_count: u8,
        current_slot: u64,
        last_reset_slot: u64,
    ) -> Result<(bool, u8, u64)> {
        let slots_elapsed = current_slot.saturating_sub(last_reset_slot);
        
        if slots_elapsed >= RATE_LIMIT_WINDOW_BLOCKS {
            // Reset window
            Ok((true, 1, current_slot))
        } else if transaction_count < MAX_TRANSACTIONS_PER_BLOCK {
            // Within limit
            Ok((true, transaction_count + 1, last_reset_slot))
        } else {
            // Rate limit exceeded
            Err(LazorKitError::RateLimitExceeded.into())
        }
    }
} 