use anchor_lang::prelude::*;

/// Transaction session for deferred execution.
/// Created after full passkey + policy verification. Contains all bindings
/// necessary to execute the transaction later without re-verification.
#[account]
#[derive(InitSpace, Debug)]
pub struct TransactionSession {
    /// Smart wallet that authorized this session
    pub owner_wallet: Pubkey,
    /// sha256 of transaction instruction data
    pub data_hash: [u8; 32],
    /// sha256 over ordered remaining account metas plus target program
    pub accounts_hash: [u8; 32],
    /// The nonce that was authorized at session creation (bound into data hash)
    pub authorized_nonce: u64,
    /// Unix expiration timestamp
    pub expires_at: i64,
    /// Where to refund rent when closing the session
    pub rent_refund_to: Pubkey,
}

impl TransactionSession {
    pub const PREFIX_SEED: &'static [u8] = b"transaction_session";
}
