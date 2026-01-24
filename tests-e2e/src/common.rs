use anyhow::{anyhow, Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::env;
use std::str::FromStr;

pub struct TestContext {
    pub client: RpcClient,
    pub payer: Keypair,
    pub program_id: Pubkey,
}

impl TestContext {
    pub fn new() -> Result<Self> {
        let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8899".to_string());
        let keypair_path = env::var("KEYPAIR")
            .unwrap_or_else(|_| shellexpand::tilde("~/.config/solana/id.json").into_owned());
        let program_id_str =
            env::var("PROGRAM_ID").expect("Please set PROGRAM_ID environment variable.");
        let program_id = Pubkey::from_str(&program_id_str)?;

        let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
        let payer = read_keypair_file(&keypair_path).expect("Failed to read keypair file");

        Ok(Self {
            client,
            payer,
            program_id,
        })
    }

    pub fn send_transaction(&self, ixs: &[Instruction], signers: &[&Keypair]) -> Result<String> {
        let latest_blockhash = self.client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&self.payer.pubkey()),
            signers,
            latest_blockhash,
        );
        let sig = self.client.send_and_confirm_transaction(&tx)?;
        Ok(sig.to_string())
    }

    pub fn send_transaction_expect_error(
        &self,
        ixs: &[Instruction],
        signers: &[&Keypair],
    ) -> Result<()> {
        let latest_blockhash = self.client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&self.payer.pubkey()),
            signers,
            latest_blockhash,
        );
        match self.client.send_and_confirm_transaction(&tx) {
            Ok(_) => Err(anyhow!("Transaction succeeded unexpectedly!")),
            Err(e) => {
                println!("Expected error received: {:?}", e);
                Ok(())
            },
        }
    }

    pub fn fund_account(&self, target: &Pubkey, lamports: u64) -> Result<()> {
        let ix = system_instruction::transfer(&self.payer.pubkey(), target, lamports);
        self.send_transaction(&[ix], &[&self.payer])?;
        Ok(())
    }
}
