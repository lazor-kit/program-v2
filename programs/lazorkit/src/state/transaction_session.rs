use anchor_lang::prelude::*;

/// Transaction session for deferred execution.
/// Created after full passkey + policy verification. Contains all bindings
/// necessary to execute the transaction later without re-verification.
#[account]
#[derive(InitSpace, Debug)]
pub struct TransactionSession {
    /// Smart wallet that authorized this session
    pub owner_wallet_address: Pubkey,
    /// Combined sha256 hash of all transaction instruction data
    pub instruction_data_hash: [u8; 32],
    /// Combined sha256 hash over all ordered remaining account metas plus target programs
    pub accounts_metadata_hash: [u8; 32],
    /// The nonce that was authorized at session creation (bound into data hash)
    pub authorized_nonce: u64,
    /// Unix expiration timestamp
    pub expires_at: i64,
    /// Where to refund rent when closing the session
    pub rent_refund_address: Pubkey,
    /// Vault index for fee collection
    pub vault_index: u8,
}

impl TransactionSession {
    pub const PREFIX_SEED: &'static [u8] = b"transaction_session";
}
