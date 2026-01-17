mod common;
use common::{bridge_tx, create_wallet, setup_env, to_sdk_hash, LiteSVMConnection};
use lazorkit_sdk::basic::actions::{
    AddAuthorityBuilder, DeactivatePolicyBuilder, RegisterPolicyBuilder,
};
use lazorkit_sdk::basic::policy::PolicyConfigBuilder;
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::{AuthorityType, PolicyRegistryEntry};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

#[test]
fn test_register_policy_happy_path() {
    let mut env = setup_env();
    let policy_id = Keypair::new().pubkey();

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        let builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(policy_id);
        futures::executor::block_on(builder.build_transaction(&connection)).unwrap()
    };

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    // Verify registry account exists
    let (registry_pda, _) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id.to_bytes()],
        &env.program_id,
    );
    let acc = env
        .svm
        .get_account(&solana_address::Address::from(registry_pda.to_bytes()))
        .unwrap();
    assert_eq!(acc.data[48], 1); // is_active
}

#[test]
fn test_deactivate_policy() {
    let mut env = setup_env();
    let policy_id = Keypair::new().pubkey();

    // 1. Register
    let reg_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        let reg_builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(policy_id);
        futures::executor::block_on(reg_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_reg_tx = Transaction::new_unsigned(reg_tx.message);
    signed_reg_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_tx)).unwrap();

    // 2. Deactivate
    let deact_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        let deact_builder = DeactivatePolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(policy_id);
        futures::executor::block_on(deact_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_deact_tx = Transaction::new_unsigned(deact_tx.message);
    signed_deact_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm
        .send_transaction(bridge_tx(signed_deact_tx))
        .unwrap();

    // Verify is_active is 0
    let (registry_pda, _) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id.to_bytes()],
        &env.program_id,
    );
    let acc = env
        .svm
        .get_account(&solana_address::Address::from(registry_pda.to_bytes()))
        .unwrap();
    assert_eq!(acc.data[48], 0);
}

#[test]
fn test_add_authority_unverified_policy_fails() {
    let mut env = setup_env();
    let wallet_id = [1u8; 32];
    let owner_kp = Keypair::from_bytes(&env.payer.to_bytes()).unwrap();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    let policy_id = Keypair::new().pubkey();
    let policy_bytes = PolicyConfigBuilder::new()
        .add_policy(policy_id, vec![]) // No state
        .build();

    let (registry_pda, _) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id.to_bytes()],
        &env.program_id,
    );

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(env.payer.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_policy_config(policy_bytes)
        .with_authorization_data(vec![1])
        .with_authorizer(env.payer.pubkey())
        .with_registry(registry_pda);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    assert!(res.is_err());
}

#[test]
fn test_add_authority_deactivated_policy_fails() {
    let mut env = setup_env();
    let wallet_id = [3u8; 32];
    let owner_kp = Keypair::from_bytes(&env.payer.to_bytes()).unwrap();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    let policy_id = Keypair::new().pubkey();

    // 1. Register and Deactivate
    let reg_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        let reg_builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(policy_id);
        futures::executor::block_on(reg_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_reg_tx = Transaction::new_unsigned(reg_tx.message);
    signed_reg_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_tx)).unwrap();

    let deact_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        let deact_builder = DeactivatePolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(policy_id);
        futures::executor::block_on(deact_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_deact_tx = Transaction::new_unsigned(deact_tx.message);
    signed_deact_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm
        .send_transaction(bridge_tx(signed_deact_tx))
        .unwrap();

    // 2. Try Add Authority
    let policy_bytes = PolicyConfigBuilder::new()
        .add_policy(policy_id, vec![])
        .build();
    let (registry_pda, _) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id.to_bytes()],
        &env.program_id,
    );

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(env.payer.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_policy_config(policy_bytes)
        .with_authorization_data(vec![1])
        .with_authorizer(env.payer.pubkey())
        .with_registry(registry_pda);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    assert!(res.is_err());
}

#[test]
fn test_add_authority_verified_policy_success() {
    let mut env = setup_env();
    let wallet_id = [4u8; 32];
    let owner_kp = Keypair::from_bytes(&env.payer.to_bytes()).unwrap();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    let policy_id = Keypair::new().pubkey();

    // 1. Register
    let reg_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        let reg_builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(policy_id);
        futures::executor::block_on(reg_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_reg_tx = Transaction::new_unsigned(reg_tx.message);
    signed_reg_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_tx)).unwrap();

    // 2. Add Authority
    let policy_bytes = PolicyConfigBuilder::new()
        .add_policy(policy_id, vec![])
        .build();
    let (registry_pda, _) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id.to_bytes()],
        &env.program_id,
    );

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(Keypair::new().pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_policy_config(policy_bytes)
        .with_authorization_data(vec![1])
        .with_authorizer(env.payer.pubkey())
        .with_registry(registry_pda);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    assert!(res.is_ok());
}
