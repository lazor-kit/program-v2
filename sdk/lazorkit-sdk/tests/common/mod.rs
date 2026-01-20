use anyhow::Result;
use async_trait::async_trait;
use lazorkit_sdk::core::connection::SolConnection;
use solana_program_test::{BanksClient, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TestContext {
    pub context: Arc<Mutex<ProgramTestContext>>,
    pub payer: Keypair,
}

impl TestContext {
    pub async fn new() -> Self {
        let program_test = ProgramTest::new(
            "lazorkit_program",
            lazorkit_program::id().into(),
            None, // processor is None because we're loading the .so or using BPF loader
                  // However, for integration tests with the actual processed code,
                  // we usually need to link the processor.
                  // Since we are testing the SDK against the on-chain program,
                  // we'll assume the program is available or mock it.
                  // For now, let's try standard ProgramTest setup.
        );
        // We assume the program is built and available as an SBF/BPF binary.
        // Solan-program-test automatically loads programs from target/deploy if we don't add them manually,
        // or we can use ProgramTest::new("lazorkit_program", ...) which tries to load the BPF.
        // Since we removed the manual add_program with processor! macro (due to incompatible types with pinocchio),
        // we rely on the SBF binary being present.

        let context = program_test.start_with_context().await;
        let payer = Keypair::from_bytes(&context.payer.to_bytes()).unwrap();

        Self {
            context: Arc::new(Mutex::new(context)),
            payer,
        }
    }

    pub async fn get_client(&self) -> BanksClient {
        self.context.lock().await.banks_client.clone()
    }

    pub async fn get_latest_blockhash(&self) -> Hash {
        self.context.lock().await.last_blockhash
    }
}

// Implement SolConnection for TestContext to use with SDK
#[async_trait]
impl SolConnection for TestContext {
    async fn get_latest_blockhash(&self) -> Result<Hash, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.context.lock().await.last_blockhash)
    }

    async fn send_transaction(
        &self,
        tx: &Transaction,
    ) -> Result<Signature, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client().await;
        // serialize transaction to get signature
        let signature = tx.signatures.first().ok_or("No signature")?;
        client
            .process_transaction(tx.clone())
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(*signature)
    }

    async fn get_account(
        &self,
        pubkey: &Pubkey,
    ) -> Result<Option<Account>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client().await;
        client
            .get_account(*pubkey)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_minimum_balance_for_rent_exemption(
        &self,
        data_len: usize,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.context.lock().await.banks_client.clone();
        let rent = client
            .get_rent()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(rent.minimum_balance(data_len))
    }
}

pub async fn setup_test_context() -> TestContext {
    TestContext::new().await
}
