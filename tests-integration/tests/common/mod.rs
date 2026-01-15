use lazorkit_program::instruction::LazorKitInstruction;
use lazorkit_state::{
    authority::{ed25519::Ed25519Authority, AuthorityType},
    IntoBytes,
};
use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::path::PathBuf;

pub struct TestEnv {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub program_id: Pubkey,
    pub sol_limit_id_pubkey: Pubkey,
    pub system_program_id: Pubkey,
}

pub fn get_program_path() -> PathBuf {
    let root = std::env::current_dir().unwrap();
    let path = root.join("target/deploy/lazorkit_program.so");
    if path.exists() {
        return path;
    }
    let path = root.join("../target/deploy/lazorkit_program.so");
    if path.exists() {
        return path;
    }
    // Try parent directory if running from tests-integration
    let path = root
        .parent()
        .unwrap()
        .join("target/deploy/lazorkit_program.so");
    if path.exists() {
        return path;
    }
    panic!("Could not find lazorkit_program.so");
}

pub fn get_sol_limit_plugin_path() -> PathBuf {
    let root = std::env::current_dir().unwrap();
    let path = root.join("target/deploy/lazorkit_sol_limit_plugin.so");
    if path.exists() {
        return path;
    }
    let path = root.join("../target/deploy/lazorkit_sol_limit_plugin.so");
    if path.exists() {
        return path;
    }
    let path = root
        .parent()
        .unwrap()
        .join("target/deploy/lazorkit_sol_limit_plugin.so");
    if path.exists() {
        return path;
    }
    panic!("Could not find lazorkit_sol_limit_plugin.so");
}

pub fn setup_env() -> TestEnv {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    // 1. Setup LazorKit Program
    let program_id_str = "LazorKit11111111111111111111111111111111111";
    let program_id = std::str::FromStr::from_str(program_id_str).unwrap();
    let program_bytes = std::fs::read(get_program_path()).expect("Failed to read program binary");
    let _ = svm.add_program(program_id, &program_bytes);

    // 2. Setup Sol Limit Plugin Program
    let sol_limit_id_pubkey = Keypair::new().pubkey();

    let plugin_bytes =
        std::fs::read(get_sol_limit_plugin_path()).expect("Failed to read sol_limit plugin binary");
    let _ = svm.add_program(sol_limit_id_pubkey, &plugin_bytes);

    let system_program_id = solana_sdk::system_program::id();

    TestEnv {
        svm,
        payer,
        program_id,
        sol_limit_id_pubkey,
        system_program_id,
    }
}

pub fn create_wallet(
    env: &mut TestEnv,
    wallet_id: [u8; 32],
    owner_kp: &Keypair,
    auth_type: AuthorityType,
) -> (Pubkey, Pubkey) {
    let (config_pda, bump) =
        Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &env.program_id);
    let (vault_pda, wallet_bump) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &env.program_id,
    );

    let owner_auth_blob = match auth_type {
        AuthorityType::Ed25519 => Ed25519Authority::new(owner_kp.pubkey().to_bytes())
            .into_bytes()
            .unwrap()
            .to_vec(),
        _ => panic!("Unsupported auth type for simple create_wallet helper"),
    };

    let create_instruction = LazorKitInstruction::CreateWallet {
        id: wallet_id,
        bump,
        wallet_bump,
        owner_authority_type: auth_type as u16,
        owner_authority_data: owner_auth_blob,
    };

    let create_ix_data = borsh::to_vec(&create_instruction).unwrap();
    let create_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
    ];
    let create_tx = Transaction::new(
        &[&env.payer],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts: create_accounts,
                data: create_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(create_tx).unwrap();

    (config_pda, vault_pda)
}
