use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;

/// Abstraction for an entity that can sign messages/transactions.
/// This allows the SDK to work with:
/// 1. Local Keypairs (Backend/CLI)
/// 2. Wallet Adapters (Frontend - Unsigned Transaction flows)
#[async_trait]
pub trait LazorSigner: Send + Sync {
    fn pubkey(&self) -> Pubkey;

    /// Sign a message.
    /// Not all signers support this (e.g. some wallet adapters might only sign transactions).
    /// Returns Err if not supported or failed.
    async fn sign_message(&self, message: &[u8]) -> Result<Signature, String>;
}
