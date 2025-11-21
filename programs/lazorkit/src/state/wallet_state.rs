use anchor_lang::prelude::*;

#[account]
#[derive(Debug)]
pub struct WalletState {
    // Core header
    pub bump: u8,        // 1
    pub wallet_id: u64,  // 8
    pub last_nonce: u64, // 8  (anti-replay cho exec)

    pub policy_program: Pubkey,
    pub policy_data: Vec<u8>,
}
impl WalletState {
    pub const PREFIX_SEED: &'static [u8] = b"wallet_state";

    pub const INIT_SPACE: usize = 1 + 8 + 8 + 32 + 4;
}
