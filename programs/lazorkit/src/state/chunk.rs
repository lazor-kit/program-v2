use anchor_lang::prelude::*;

/// Transaction chunk for deferred execution
///
/// Created after full passkey and policy verification. Contains all bindings
/// necessary to execute the transaction later without re-verification.
/// Used for large transactions that need to be split into manageable chunks.
#[account]
#[derive(InitSpace, Debug)]
pub struct Chunk {
    /// Smart wallet address that authorized this chunk session
    pub owner_wallet_address: Pubkey,
    /// Combined SHA256 hash of all cpi transaction instruction data
    pub cpi_hash: [u8; 32],
    /// The nonce that was authorized at chunk creation (bound into data hash)
    pub authorized_nonce: u64,
    /// Timestamp from the original message hash for expiration validation
    pub authorized_timestamp: i64,
    /// Address to receive rent refund when closing the chunk session
    pub rent_refund_address: Pubkey,
    /// Vault index for fee collection during chunk execution
    pub vault_index: u8,
}

impl Chunk {
    /// Seed prefix used for PDA derivation of chunk accounts
    pub const PREFIX_SEED: &'static [u8] = b"chunk";
}
