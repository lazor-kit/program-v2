use anchor_lang::prelude::*;

/// Ephemeral authorization for temporary program access.
/// Created after passkey authentication. Allows execution with ephemeral key
/// for a limited time to authorized programs with multiple instructions.
#[account]
#[derive(InitSpace, Debug)]
pub struct EphemeralAuthorization {
    /// Smart wallet that authorized this session
    pub owner_wallet_address: Pubkey,
    /// Ephemeral public key that can sign transactions
    pub ephemeral_public_key: Pubkey,
    /// Unix timestamp when this session expires
    pub expires_at: i64,
    /// Fee payer for transactions in this session
    pub fee_payer_address: Pubkey,
    /// Where to refund rent when closing the session
    pub rent_refund_address: Pubkey,
    /// Vault index for fee collection
    pub vault_index: u8,
    /// Combined hash of all instruction data that can be executed
    pub instruction_data_hash: [u8; 32],
    /// Combined hash of all accounts that will be used
    pub accounts_metadata_hash: [u8; 32],
}

impl EphemeralAuthorization {
    pub const PREFIX_SEED: &'static [u8] = b"ephemeral_authorization";
}
