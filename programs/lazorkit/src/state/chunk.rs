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
    /// Combined SHA256 hash of all transaction instruction data
    pub instruction_data_hash: [u8; 32],
    /// Combined SHA256 hash over all ordered remaining account metas plus target programs
    pub accounts_metadata_hash: [u8; 32],
    /// The nonce that was authorized at chunk creation (bound into data hash)
    pub authorized_nonce: u64,
    /// Unix timestamp when this chunk expires
    pub expires_at: i64,
    /// Address to receive rent refund when closing the chunk session
    pub rent_refund_address: Pubkey,
    /// Vault index for fee collection during chunk execution
    pub vault_index: u8,
}

impl Chunk {
    /// Seed prefix used for PDA derivation of chunk accounts
    pub const PREFIX_SEED: &'static [u8] = b"chunk";
}
