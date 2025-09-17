use anchor_lang::prelude::*;

/// Ephemeral authorization for temporary program access
///
/// Created after passkey authentication to allow execution with an ephemeral key
/// for a limited time. Enables multiple operations without repeated passkey
/// authentication, ideal for games and applications requiring frequent interactions.
#[account]
#[derive(InitSpace, Debug)]
pub struct Permission {
    /// Smart wallet address that authorized this permission session
    pub owner_wallet_address: Pubkey,
    /// Ephemeral public key that can sign transactions during this session
    pub ephemeral_public_key: Pubkey,
    /// Unix timestamp when this permission session expires
    pub expires_at: i64,
    /// Fee payer address for transactions in this session
    pub fee_payer_address: Pubkey,
    /// Address to receive rent refund when closing the session
    pub rent_refund_address: Pubkey,
    /// Vault index for fee collection during this session
    pub vault_index: u8,
    /// Combined hash of all instruction data that can be executed
    pub instruction_data_hash: [u8; 32],
    /// Combined hash of all accounts that will be used in this session
    pub accounts_metadata_hash: [u8; 32],
}

impl Permission {
    /// Seed prefix used for PDA derivation of permission accounts
    pub const PREFIX_SEED: &'static [u8] = b"permission";
}
