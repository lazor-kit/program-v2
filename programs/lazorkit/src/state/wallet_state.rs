use anchor_lang::prelude::*;

/// Wallet state account storing wallet configuration and execution state
#[account]
#[derive(Debug)]
pub struct WalletState {
    /// PDA bump seed for smart wallet
    pub bump: u8,
    /// Unique wallet identifier
    pub wallet_id: u64,
    /// Last used nonce for anti-replay protection
    pub last_nonce: u64,

    /// Policy program that validates transactions
    pub policy_program: Pubkey,
    /// Serialized policy data returned from policy initialization
    pub policy_data: Vec<u8>,
}
impl WalletState {
    pub const PREFIX_SEED: &'static [u8] = b"wallet_state";

    pub const INIT_SPACE: usize = 1 + 8 + 8 + 32 + 4;
}
