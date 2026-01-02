mod call_policy;
mod change_policy;

use crate::constants::PASSKEY_PUBLIC_KEY_SIZE;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::HASH_BYTES;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct NewWalletAuthority {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub credential_hash: [u8; HASH_BYTES],
}

pub use call_policy::*;
pub use change_policy::*;
