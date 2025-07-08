/// Common types shared across instruction handlers
use anchor_lang::prelude::*;

/// Data describing an instruction that will be invoked via CPI.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CpiData {
    /// Raw instruction data (Anchor discriminator + serialized args)
    pub data: Vec<u8>,
    /// Starting index in `remaining_accounts` for the accounts slice
    pub start_index: u8,
    /// Number of accounts to take from `remaining_accounts`
    pub length: u8,
} 