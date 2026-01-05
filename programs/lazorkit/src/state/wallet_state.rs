use anchor_lang::{prelude::*, solana_program::pubkey::PUBKEY_BYTES};
use core::mem::size_of;

/// Wallet state account storing wallet configuration and execution state
#[account]
#[derive(Debug)]
pub struct WalletState {
    /// PDA bump seed for smart wallet
    pub bump: u8,
    /// Last used nonce for anti-replay protection
    pub last_nonce: u64,

    /// Base seed for smart wallet address derivation (initial credential_hash)
    pub base_seed: [u8; 32],

    /// Salt for smart wallet address derivation
    pub salt: u64,

    /// Policy program that validates transactions
    pub policy_program: Pubkey,
    /// Serialized policy data returned from policy initialization
    pub policy_data: Vec<u8>,
}
impl WalletState {
    pub const PREFIX_SEED: &'static [u8] = b"wallet_state";

    pub const INIT_SPACE: usize =
        size_of::<u8>() + size_of::<u64>() + 32 + size_of::<u64>() + PUBKEY_BYTES + 4;
}
