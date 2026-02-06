use anyhow::{anyhow, Result};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction::Transaction;

pub struct TestContext {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub program_id: Pubkey,
}

impl TestContext {
    pub fn new() -> Result<Self> {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();

        // Airdrop via Address conversion
        let payer_pubkey = payer.pubkey();
        let payer_addr = Address::from(payer_pubkey.to_bytes());
        svm.airdrop(&payer_addr, 1_000_000_000_000).unwrap();

        // Load and deploy program
        let program_data = include_bytes!("../../target/deploy/lazorkit_program.so");
        let program_keypair_data =
            include_bytes!("../../target/deploy/lazorkit_program-keypair.json");
        let program_keypair_bytes: Vec<u8> = serde_json::from_slice(program_keypair_data)
            .map_err(|e| anyhow::anyhow!("Failed to parse program keypair: {}", e))?;

        // Solana keypair JSON contains 64 bytes [secret_key(32) + public_key(32)]
        // new_from_array expects only the secret key (first 32 bytes)
        let mut secret_key = [0u8; 32];
        secret_key.copy_from_slice(&program_keypair_bytes[..32]);
        let _program_keypair = Keypair::new_from_array(secret_key);
        // Extract Pubkey directly from the bytes (last 32 bytes)
        let program_id = Pubkey::try_from(&program_keypair_bytes[32..64])
            .map_err(|e| anyhow::anyhow!("Failed to create program pubkey: {}", e))?;

        svm.add_program(program_id.to_address(), program_data)
            .map_err(|e| anyhow::anyhow!("Failed to add program: {:?}", e))?;

        Ok(Self {
            svm,
            payer,
            program_id, // program_id is already Pubkey
        })
    }

    // Execute a pre-built transaction
    pub fn execute_tx(&mut self, tx: Transaction) -> Result<String> {
        // Convert Transaction -> VersionedTransaction
        let v_tx = VersionedTransaction::from(tx);
        let result = self
            .svm
            .send_transaction(v_tx)
            .map_err(|e| anyhow!("Transaction failed: {:?}", e))?;
        Ok(format!("{:?}", result))
    }

    pub fn execute_tx_expect_error(&mut self, tx: Transaction) -> Result<()> {
        let v_tx = VersionedTransaction::from(tx);
        match self.svm.send_transaction(v_tx) {
            Ok(_) => Err(anyhow!("Transaction succeeded unexpectedly!")),
            Err(e) => {
                println!("Expected error received: {:?}", e);
                Ok(())
            },
        }
    }

    pub fn get_account(&mut self, pubkey: &Pubkey) -> Result<Account> {
        let addr = Address::from(pubkey.to_bytes());
        self.svm
            .get_account(&addr)
            .ok_or_else(|| anyhow!("Account not found"))
    }
}

pub trait ToAddress {
    fn to_address(&self) -> Address;
}

impl ToAddress for Pubkey {
    fn to_address(&self) -> Address {
        Address::from(self.to_bytes())
    }
}

impl ToAddress for Address {
    fn to_address(&self) -> Address {
        *self
    }
}
