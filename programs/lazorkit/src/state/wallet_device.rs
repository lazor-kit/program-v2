use crate::constants::PASSKEY_PUBLIC_KEY_SIZE;
use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct WalletDevice {
    pub passkey_pubkey: [u8; PASSKEY_PUBLIC_KEY_SIZE],

    pub credential_hash: [u8; 32],

    pub smart_wallet: Pubkey,

    pub bump: u8,
}

impl WalletDevice {
    pub const PREFIX_SEED: &'static [u8] = b"wallet_device";
}
