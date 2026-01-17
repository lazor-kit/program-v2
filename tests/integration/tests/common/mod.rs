#[allow(dead_code)]
use lazorkit_sdk::basic::actions::CreateWalletBuilder;
use lazorkit_sdk::core::connection::SolConnection;
use lazorkit_sdk::state::AuthorityType;
use litesvm::LiteSVM;
use solana_address::Address;
use solana_sdk::{
    account::Account,
    hash::Hash,
    pubkey::Pubkey,
    signature::Signature,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::error::Error;
use std::path::PathBuf;

pub struct TestEnv {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub program_id: Pubkey,
    pub sol_limit_id_pubkey: Pubkey,
}

pub struct LiteSVMConnection<'a> {
    pub svm: &'a LiteSVM,
}

#[async_trait::async_trait]
impl<'a> SolConnection for LiteSVMConnection<'a> {
    async fn send_transaction(
        &self,
        _tx: &Transaction,
    ) -> Result<Signature, Box<dyn Error + Send + Sync>> {
        unimplemented!("LiteSVMConnection::send_transaction not needed for build_transaction")
    }
    async fn get_account(
        &self,
        _pubkey: &Pubkey,
    ) -> Result<Option<Account>, Box<dyn Error + Send + Sync>> {
        unimplemented!()
    }
    async fn get_latest_blockhash(&self) -> Result<Hash, Box<dyn Error + Send + Sync>> {
        Ok(to_sdk_hash(self.svm.latest_blockhash()))
    }
    async fn get_minimum_balance_for_rent_exemption(
        &self,
        _data_len: usize,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(0)
    }
}

pub fn get_program_path() -> PathBuf {
    let root = std::env::current_dir().unwrap();
    let paths = [
        root.join("target/deploy/lazorkit_program.so"),
        root.join("../target/deploy/lazorkit_program.so"),
        root.join("../../target/deploy/lazorkit_program.so"),
    ];
    for path in paths {
        if path.exists() {
            return path;
        }
    }
    panic!("Could not find lazorkit_program.so");
}

pub fn get_sol_limit_plugin_path() -> PathBuf {
    let root = std::env::current_dir().unwrap();
    let paths = [
        root.join("target/deploy/lazorkit_sol_limit_plugin.so"),
        root.join("../target/deploy/lazorkit_sol_limit_plugin.so"),
        root.join("../../target/deploy/lazorkit_sol_limit_plugin.so"),
    ];
    for path in paths {
        if path.exists() {
            return path;
        }
    }
    panic!("Could not find lazorkit_sol_limit_plugin.so");
}

// Helper for Hash
pub fn to_sdk_hash(h: solana_hash::Hash) -> solana_sdk::hash::Hash {
    solana_sdk::hash::Hash::new_from_array(h.to_bytes())
}

// Helper to bridge SDK Transaction to Litesvm (VersionedTransaction)
pub fn bridge_tx(tx: Transaction) -> solana_transaction::versioned::VersionedTransaction {
    let bytes = bincode::serialize(&tx).unwrap();
    bincode::deserialize(&bytes).unwrap()
}

pub fn setup_env() -> TestEnv {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&Address::from(payer.pubkey().to_bytes()), 10_000_000_000)
        .unwrap();

    // 1. Setup LazorKit Program
    let program_id_str = "LazorKit11111111111111111111111111111111111";
    let program_id: Pubkey = std::str::FromStr::from_str(program_id_str).unwrap();
    let program_bytes = std::fs::read(get_program_path()).expect("Failed to read program binary");
    let _ = svm.add_program(Address::from(program_id.to_bytes()), &program_bytes);

    // 2. Setup Sol Limit Plugin Program
    let sol_limit_id_pubkey = Keypair::new().pubkey();
    let plugin_bytes =
        std::fs::read(get_sol_limit_plugin_path()).expect("Failed to read sol_limit plugin binary");
    let _ = svm.add_program(Address::from(sol_limit_id_pubkey.to_bytes()), &plugin_bytes);

    // let system_program_id = solana_sdk::system_program::id();

    TestEnv {
        svm,
        payer,
        program_id,
        sol_limit_id_pubkey,
    }
}

pub fn create_wallet(
    env: &mut TestEnv,
    wallet_id: [u8; 32],
    owner_kp: &Keypair,
    _auth_type: AuthorityType,
) -> (Pubkey, Pubkey) {
    let connection = LiteSVMConnection { svm: &env.svm };
    let builder = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_owner(owner_kp.pubkey())
        .with_id(wallet_id);

    let tx = futures::executor::block_on(builder.build_transaction(&connection)).unwrap();

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    // Re-derive PDAs for convenience return (builders don't return them currently)
    // Actually we should probably make LazorWallet::new more accessible
    let (config_pda, _) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &env.program_id);
    let (vault_pda, _) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &env.program_id,
    );

    (config_pda, vault_pda)
}
