use anchor_lang::prelude::*;

/// Commit record for a future CPI execution.
/// Created after full passkey + rule verification. Contains all bindings
/// necessary to perform the CPI later without re-verification.
#[account]
#[derive(InitSpace, Debug)]
pub struct CpiCommit {
    /// Smart wallet that authorized this commit
    pub owner_wallet: Pubkey,
    /// Target program id for the CPI
    pub target_program: Pubkey,
    /// sha256 of CPI instruction data
    pub data_hash: [u8; 32],
    /// sha256 over ordered remaining account metas plus `target_program`
    pub accounts_hash: [u8; 32],
    /// The nonce that was authorized at commit time (bound into data hash)
    pub authorized_nonce: u64,
    /// Unix expiration timestamp
    pub expires_at: i64,
    /// Where to refund rent when closing the commit
    pub rent_refund_to: Pubkey,
}

impl CpiCommit {
    pub const PREFIX_SEED: &'static [u8] = b"cpi_commit";
}


