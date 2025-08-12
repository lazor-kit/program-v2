use anchor_lang::prelude::*;

use crate::constants::PASSKEY_SIZE;

/// Event emitted when a new smart wallet is created
#[event]
pub struct SmartWalletCreated {
    pub smart_wallet: Pubkey,
    pub authenticator: Pubkey,
    pub sequence_id: u64,
    pub rule_program: Pubkey,
    pub passkey_hash: [u8; 32],
    pub timestamp: i64,
}

/// Event emitted when a transaction is executed
#[event]
pub struct TransactionExecuted {
    pub smart_wallet: Pubkey,
    pub authenticator: Pubkey,
    pub nonce: u64,
    pub rule_program: Pubkey,
    pub cpi_program: Pubkey,
    pub success: bool,
    pub timestamp: i64,
}

/// Event emitted when a rule program is changed
#[event]
pub struct RuleProgramChanged {
    pub smart_wallet: Pubkey,
    pub old_rule_program: Pubkey,
    pub new_rule_program: Pubkey,
    pub nonce: u64,
    pub timestamp: i64,
}

/// Event emitted when a new authenticator is added
#[event]
pub struct AuthenticatorAdded {
    pub smart_wallet: Pubkey,
    pub new_authenticator: Pubkey,
    pub passkey_hash: [u8; 32],
    pub added_by: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when program configuration is updated
#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
    pub update_type: String,
    pub old_value: String,
    pub new_value: String,
    pub timestamp: i64,
}

/// Event emitted when program is initialized
#[event]
pub struct ProgramInitialized {
    pub authority: Pubkey,
    pub default_rule_program: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when a fee is collected
#[event]
pub struct FeeCollected {
    pub smart_wallet: Pubkey,
    pub fee_type: String,
    pub amount: u64,
    pub recipient: Pubkey,
    pub timestamp: i64,
}

/// Event emitted when program is paused/unpaused
#[event]
pub struct ProgramPausedStateChanged {
    pub authority: Pubkey,
    pub is_paused: bool,
    pub timestamp: i64,
}

/// Event emitted when a whitelist rule program is added
#[event]
pub struct WhitelistRuleProgramAdded {
    pub authority: Pubkey,
    pub rule_program: Pubkey,
    pub timestamp: i64,
}

/// Event emitted for security-related events
#[event]
pub struct SecurityEvent {
    pub event_type: String,
    pub smart_wallet: Option<Pubkey>,
    pub details: String,
    pub severity: String,
    pub timestamp: i64,
}

/// Event emitted when a SOL transfer occurs
#[event]
pub struct SolTransfer {
    pub smart_wallet: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
    pub nonce: u64,
    pub timestamp: i64,
}

/// Event emitted for errors that are caught and handled
#[event]
pub struct ErrorEvent {
    pub smart_wallet: Option<Pubkey>,
    pub error_code: String,
    pub error_message: String,
    pub action_attempted: String,
    pub timestamp: i64,
}

// Helper functions for emitting events

impl SmartWalletCreated {
    pub fn emit_event(
        smart_wallet: Pubkey,
        authenticator: Pubkey,
        sequence_id: u64,
        rule_program: Pubkey,
        passkey_pubkey: [u8; PASSKEY_SIZE],
    ) -> Result<()> {
        let mut passkey_hash = [0u8; 32];
        passkey_hash.copy_from_slice(&anchor_lang::solana_program::hash::hash(&passkey_pubkey).to_bytes()[..32]);
        
        emit!(Self {
            smart_wallet,
            authenticator,
            sequence_id,
            rule_program,
            passkey_hash,
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }
}

impl TransactionExecuted {
    pub fn emit_event(
        smart_wallet: Pubkey,
        authenticator: Pubkey,
        nonce: u64,
        rule_program: Pubkey,
        cpi_program: Pubkey,
        success: bool,
    ) -> Result<()> {
        emit!(Self {
            smart_wallet,
            authenticator,
            nonce,
            rule_program,
            cpi_program,
            success,
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }
}

impl SecurityEvent {
    pub fn emit_warning(
        smart_wallet: Option<Pubkey>,
        event_type: &str,
        details: &str,
    ) -> Result<()> {
        emit!(Self {
            event_type: event_type.to_string(),
            smart_wallet,
            details: details.to_string(),
            severity: "WARNING".to_string(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }
    
    pub fn emit_critical(
        smart_wallet: Option<Pubkey>,
        event_type: &str,
        details: &str,
    ) -> Result<()> {
        emit!(Self {
            event_type: event_type.to_string(),
            smart_wallet,
            details: details.to_string(),
            severity: "CRITICAL".to_string(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        Ok(())
    }
} 