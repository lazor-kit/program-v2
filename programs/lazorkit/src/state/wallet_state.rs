use anchor_lang::prelude::*;

use crate::constants::{MAX_DEVICE_COUNT, MAX_POLICY_BYTES, PASSKEY_PUBLIC_KEY_SIZE};

#[account]
#[derive(Debug, InitSpace)]
pub struct WalletState {
    // Core header
    pub bump: u8,         // 1
    pub wallet_id: u64,   // 8
    pub last_nonce: u64,  // 8  (anti-replay cho exec)
    pub referral: Pubkey, // 32

    pub policy_program: Pubkey, // 2 + 32
    pub policy_data_len: u16,   // 2
    #[max_len(MAX_POLICY_BYTES)]
    pub policy_data: Vec<u8>, // 4 + len(policy_data)

    // Devices (≤3) — O(1)
    pub device_count: u8,
    #[max_len(MAX_DEVICE_COUNT)]
    pub devices: Vec<DeviceSlot>, // ~3 * 66 = 198
}
impl WalletState {
    pub const PREFIX_SEED: &'static [u8] = b"wallet_state";
}

#[derive(Debug, InitSpace, AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DeviceSlot {
    pub passkey_pubkey: [u8; PASSKEY_PUBLIC_KEY_SIZE], // secp256r1 compressed
    pub credential_hash: [u8; 32],                     // blake3(credential_id) | 0 if not used
}
