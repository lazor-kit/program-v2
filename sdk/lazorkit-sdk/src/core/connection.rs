use async_trait::async_trait;
use solana_sdk::account::Account;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::Transaction;
use std::error::Error;

#[async_trait]
pub trait SolConnection: Send + Sync {
    async fn send_transaction(
        &self,
        tx: &Transaction,
    ) -> Result<Signature, Box<dyn Error + Send + Sync>>;
    async fn get_account(
        &self,
        pubkey: &Pubkey,
    ) -> Result<Option<Account>, Box<dyn Error + Send + Sync>>;
    async fn get_latest_blockhash(&self) -> Result<Hash, Box<dyn Error + Send + Sync>>;
    async fn get_minimum_balance_for_rent_exemption(
        &self,
        data_len: usize,
    ) -> Result<u64, Box<dyn Error + Send + Sync>>;
}
