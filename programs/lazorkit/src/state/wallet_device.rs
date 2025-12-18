use crate::constants::PASSKEY_PUBLIC_KEY_SIZE;
use anchor_lang::{prelude::*, solana_program::hash::HASH_BYTES};

/// Wallet device account linking a passkey to a smart wallet
#[account]
#[derive(Debug, InitSpace)]
pub struct WalletDevice {
    /// Secp256r1 compressed public key (33 bytes)
    pub passkey_pubkey: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// SHA256 hash of the credential ID
    pub credential_hash: [u8; HASH_BYTES],
    /// Associated smart wallet address
    pub smart_wallet: Pubkey,
    /// PDA bump seed
    pub bump: u8,
}

impl WalletDevice {
    pub const PREFIX_SEED: &'static [u8] = b"wallet_device";
}
