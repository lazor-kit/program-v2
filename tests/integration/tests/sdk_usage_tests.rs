use async_trait::async_trait;
use lazorkit_sdk::basic::actions::{AddAuthorityBuilder, CreateWalletBuilder, ExecuteBuilder};
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::core::{connection::SolConnection, signer::LazorSigner};
use lazorkit_state::authority::AuthorityType;
use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::account::Account;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature, Signer};
use solana_sdk::transaction::Transaction;
use std::error::Error;
use tokio::sync::Mutex;

struct TestConnection {
    client: Mutex<BanksClient>,
}

impl TestConnection {
    fn new(client: BanksClient) -> Self {
        Self {
            client: Mutex::new(client),
        }
    }
}

#[async_trait]
impl SolConnection for TestConnection {
    async fn send_transaction(
        &self,
        tx: &Transaction,
    ) -> Result<Signature, Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;
        client
            .process_transaction(tx.clone())
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(tx.signatures[0])
    }

    async fn get_account(
        &self,
        pubkey: &Pubkey,
    ) -> Result<Option<Account>, Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;
        client
            .get_account(*pubkey)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    }

    async fn get_latest_blockhash(&self) -> Result<Hash, Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;
        client
            .get_latest_blockhash()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
    }

    async fn get_minimum_balance_for_rent_exemption(
        &self,
        data_len: usize,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let client = self.client.lock().await;
        let rent = client
            .get_rent()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        Ok(rent.minimum_balance(data_len))
    }
}

pub struct TestSigner {
    keypair: Keypair,
}

impl TestSigner {
    pub fn new() -> Self {
        Self {
            keypair: Keypair::new(),
        }
    }
}

#[async_trait]
impl LazorSigner for TestSigner {
    fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
    async fn sign_message(&self, message: &[u8]) -> Result<Signature, String> {
        Ok(self.keypair.sign_message(message))
    }
}

#[tokio::test]
async fn test_sdk_usage_high_level() {
    let pt = ProgramTest::new("lazorkit_program", LazorWallet::DEFAULT_PROGRAM_ID, None);
    let (banks_client, payer, recent_blockhash) = pt.start().await;
    let connection = TestConnection::new(banks_client);

    // 1. Create Wallet
    let owner_kp = Keypair::new();
    let wallet_id = [1u8; 32];

    let create_builder = CreateWalletBuilder::new()
        .with_id(wallet_id)
        .with_payer(payer.pubkey())
        .with_owner_authority_type(AuthorityType::Ed25519)
        .with_owner_authority_key(owner_kp.pubkey().to_bytes().to_vec());

    let create_tx = create_builder.build_transaction(&connection).await.unwrap();
    let mut signed_create_tx = Transaction::new_unsigned(create_tx.message);
    signed_create_tx.sign(&[&payer], recent_blockhash);
    connection
        .send_transaction(&signed_create_tx)
        .await
        .unwrap();

    let (config_pda, vault_pda) = create_builder.get_pdas();
    let wallet = LazorWallet::new(LazorWallet::DEFAULT_PROGRAM_ID, config_pda, vault_pda);

    // 2. Add Authority
    let new_auth_kp = Keypair::new();
    let add_builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(new_auth_kp.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_authorization_data(vec![3])
        .with_authorizer(owner_kp.pubkey());

    let add_tx = add_builder
        .build_transaction(&connection, payer.pubkey())
        .await
        .unwrap();
    let mut signed_add_tx = Transaction::new_unsigned(add_tx.message);
    signed_add_tx.sign(&[&payer, &owner_kp], recent_blockhash);
    connection.send_transaction(&signed_add_tx).await.unwrap();

    // 3. Execute
    let recipient = Keypair::new().pubkey();
    let target_ix = solana_sdk::system_instruction::transfer(&vault_pda, &recipient, 1000);

    // Airdrop to vault
    let fund_ix =
        solana_sdk::system_instruction::transfer(&payer.pubkey(), &vault_pda, 1_000_000_000);
    let mut fund_tx = Transaction::new_with_payer(&[fund_ix], Some(&payer.pubkey()));
    fund_tx.sign(&[&payer], recent_blockhash);
    connection.send_transaction(&fund_tx).await.unwrap();

    let exec_builder = ExecuteBuilder::new(&wallet)
        .with_role_id(0)
        .add_instruction(target_ix)
        .with_signer(owner_kp.pubkey());

    let exec_tx = exec_builder
        .build_transaction(&connection, payer.pubkey())
        .await
        .unwrap();
    let mut signed_exec_tx = Transaction::new_unsigned(exec_tx.message);
    signed_exec_tx.sign(&[&payer, &owner_kp], recent_blockhash);
    connection.send_transaction(&signed_exec_tx).await.unwrap();

    // Verify recipient balance
    let recipient_acc = connection.get_account(&recipient).await.unwrap().unwrap();
    assert_eq!(recipient_acc.lamports, 1000);
}
