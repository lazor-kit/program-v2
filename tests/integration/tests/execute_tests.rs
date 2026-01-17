mod common;
use common::{bridge_tx, create_wallet, setup_env, to_sdk_hash, LiteSVMConnection};
use lazorkit_sdk::advanced::types::IntoBytes;
use lazorkit_sdk::basic::actions::{AddAuthorityBuilder, ExecuteBuilder, RegisterPolicyBuilder};
use lazorkit_sdk::basic::policy::PolicyConfigBuilder;
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::AuthorityType;
use lazorkit_sol_limit_plugin::SolLimitState;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::path::PathBuf;

pub fn get_whitelist_policy_path() -> PathBuf {
    let root = std::env::current_dir().unwrap();
    let path = root.join("target/deploy/lazorkit_whitelist_plugin.so");
    if path.exists() {
        return path;
    }
    let path = root.join("../target/deploy/lazorkit_whitelist_plugin.so");
    if path.exists() {
        return path;
    }
    let path = root
        .parent()
        .unwrap()
        .join("target/deploy/lazorkit_whitelist_plugin.so");
    if path.exists() {
        return path;
    }
    panic!("Could not find lazorkit_whitelist_plugin.so");
}

#[test]
fn test_execute_flow_with_whitelist() {
    let mut env = setup_env();

    // 1. Deploy & Register Policy
    let whitelist_policy_id = Keypair::new().pubkey();
    let policy_bytes =
        std::fs::read(get_whitelist_policy_path()).expect("Failed to read whitelist policy binary");
    env.svm
        .add_program(
            solana_address::Address::from(whitelist_policy_id.to_bytes()),
            &policy_bytes,
        )
        .unwrap();

    let reg_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        let reg_builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(whitelist_policy_id);
        futures::executor::block_on(reg_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_reg_tx = Transaction::new_unsigned(reg_tx.message);
    signed_reg_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_tx)).unwrap();

    // 2. Create Wallet
    let owner_kp = Keypair::new();
    let wallet_id = [1u8; 32];
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);
    env.svm
        .airdrop(
            &solana_address::Address::from(vault_pda.to_bytes()),
            1_000_000_000,
        )
        .unwrap();

    // 3. Add Authority with Whitelist Policy
    let whitelist_state_bytes = vec![0u8; 3204]; // 2 + 2 + 32*100
    let policy_config = PolicyConfigBuilder::new()
        .add_policy(whitelist_policy_id, whitelist_state_bytes)
        .build();

    let delegate_kp = Keypair::new();
    let (registry_pda, _) = Pubkey::find_program_address(
        &[
            lazorkit_sdk::state::PolicyRegistryEntry::SEED_PREFIX,
            &whitelist_policy_id.to_bytes(),
        ],
        &env.program_id,
    );

    let add_builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(delegate_kp.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_policy_config(policy_config)
        .with_authorization_data(vec![3])
        .with_authorizer(owner_kp.pubkey())
        .with_registry(registry_pda);

    let add_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(add_builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_add_tx = Transaction::new_unsigned(add_tx.message);
    signed_add_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_add_tx)).unwrap();

    // 4. Execute (Target: Transfer)
    let recipient = Keypair::new().pubkey();
    let transfer_amount = 1000;
    let target_ix = system_instruction::transfer(&vault_pda, &recipient, transfer_amount);

    let exec_builder = ExecuteBuilder::new(&wallet)
        .with_role_id(1)
        .add_instruction(target_ix)
        .with_signer(delegate_kp.pubkey())
        .with_policy(whitelist_policy_id);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(exec_builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &delegate_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    // Expect failure as whitelist is empty
    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    assert!(res.is_err());
}

#[test]
fn test_execute_flow_with_sol_limit() {
    let mut env = setup_env();

    // 1. Register
    let reg_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        let reg_builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(env.sol_limit_id_pubkey);
        futures::executor::block_on(reg_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_reg_tx = Transaction::new_unsigned(reg_tx.message);
    signed_reg_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_tx)).unwrap();

    // 2. Create Wallet
    let owner_kp = Keypair::new();
    let wallet_id = [2u8; 32];
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);
    env.svm
        .airdrop(
            &solana_address::Address::from(vault_pda.to_bytes()),
            10_000_000_000,
        )
        .unwrap();

    // 3. Add Authority with SolLimit
    let limit_state = SolLimitState { amount: 2000 };
    let policy_config = PolicyConfigBuilder::new()
        .add_policy(
            env.sol_limit_id_pubkey,
            limit_state.into_bytes().unwrap().to_vec(),
        )
        .build();
    let (registry_pda, _) = Pubkey::find_program_address(
        &[
            lazorkit_sdk::state::PolicyRegistryEntry::SEED_PREFIX,
            &env.sol_limit_id_pubkey.to_bytes(),
        ],
        &env.program_id,
    );

    let delegate_kp = Keypair::new();
    let add_builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(delegate_kp.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_policy_config(policy_config)
        .with_authorization_data(vec![3])
        .with_authorizer(owner_kp.pubkey())
        .with_registry(registry_pda);

    let add_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(add_builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_add_tx = Transaction::new_unsigned(add_tx.message);
    signed_add_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_add_tx)).unwrap();

    // 4. Success Execution (1000)
    let recipient = Keypair::new().pubkey();
    let target_ix = system_instruction::transfer(&vault_pda, &recipient, 1000);

    let exec_builder = ExecuteBuilder::new(&wallet)
        .with_role_id(1)
        .add_instruction(target_ix)
        .with_signer(delegate_kp.pubkey())
        .with_policy(env.sol_limit_id_pubkey);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(exec_builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &delegate_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm
        .send_transaction(bridge_tx(signed_tx))
        .expect("Execute 1000 failed");

    // 5. Fail Execution (1500)
    let target_ix2 = system_instruction::transfer(&vault_pda, &Keypair::new().pubkey(), 1500);
    let exec_builder2 = ExecuteBuilder::new(&wallet)
        .with_role_id(1)
        .add_instruction(target_ix2)
        .with_signer(delegate_kp.pubkey())
        .with_policy(env.sol_limit_id_pubkey);

    let tx2 = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(
            exec_builder2.build_transaction(&connection, env.payer.pubkey()),
        )
        .unwrap()
    };
    let mut signed_tx2 = Transaction::new_unsigned(tx2.message);
    signed_tx2
        .try_sign(
            &[&env.payer, &delegate_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    assert!(env.svm.send_transaction(bridge_tx(signed_tx2)).is_err());
}
