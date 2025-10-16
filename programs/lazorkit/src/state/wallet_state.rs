use anchor_lang::prelude::*;

use crate::constants::MAX_POLICY_BYTES;

#[account]
#[derive(Debug, InitSpace)]
pub struct WalletState {
    // Core header
    pub bump: u8,         // 1
    pub wallet_id: u64,   // 8
    pub last_nonce: u64,  // 8  (anti-replay cho exec)
    pub referral: Pubkey, // 32

    pub policy_program: Pubkey, // 2 + 32
    #[max_len(MAX_POLICY_BYTES)]
    pub policy_data: Vec<u8>, // 4 + len(policy_data)
}
impl WalletState {
    pub const PREFIX_SEED: &'static [u8] = b"wallet_state";
}
