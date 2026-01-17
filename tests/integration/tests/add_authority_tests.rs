mod common;
use common::{bridge_tx, create_wallet, setup_env, to_sdk_hash, LiteSVMConnection};
use lazorkit_sdk::advanced::types::{IntoBytes, Transmutable};
use lazorkit_sdk::basic::actions::{AddAuthorityBuilder, RegisterPolicyBuilder};
use lazorkit_sdk::basic::policy::PolicyConfigBuilder;
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::{AuthorityType, LazorKitWallet, Position};
use lazorkit_sol_limit_plugin::SolLimitState;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

#[test]
fn test_add_authority_success_with_sol_limit_policy() {
    let mut env = setup_env();
    let wallet_id = [20u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    let new_auth_kp = Keypair::new();

    // 1. Register Policy
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

    // 2. Add Authority with Policy
    let limit_state = SolLimitState {
        amount: 5_000_000_000,
    };
    let policy_bytes = PolicyConfigBuilder::new()
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

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(new_auth_kp.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_policy_config(policy_bytes)
        .with_authorization_data(vec![3]) // Authorizer at index 3
        .with_authorizer(owner_kp.pubkey())
        .with_registry(registry_pda);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    // Verify
    let acc = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = acc.data;
    let wallet_header =
        unsafe { LazorKitWallet::load_unchecked(&data[0..LazorKitWallet::LEN]).unwrap() };
    assert_eq!(wallet_header.role_count, 2);

    let pos0 = unsafe { Position::load_unchecked(&data[LazorKitWallet::LEN..]).unwrap() };
    let role1_pos = unsafe { Position::load_unchecked(&data[pos0.boundary as usize..]).unwrap() };
    assert_eq!(role1_pos.id, 1);
    assert_eq!(role1_pos.num_policies, 1);
}

#[test]
fn test_add_authority_success_ed25519_no_policies() {
    let mut env = setup_env();
    let wallet_id = [21u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    let new_auth_kp = Keypair::new();

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(new_auth_kp.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_authorization_data(vec![3])
        .with_authorizer(owner_kp.pubkey());

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    let acc = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = acc.data;
    let wallet_header =
        unsafe { LazorKitWallet::load_unchecked(&data[0..LazorKitWallet::LEN]).unwrap() };

    let pos0 = unsafe { Position::load_unchecked(&data[LazorKitWallet::LEN..]).unwrap() };
    let role1_pos = unsafe { Position::load_unchecked(&data[pos0.boundary as usize..]).unwrap() };
    assert_eq!(role1_pos.id, 1);
    assert_eq!(role1_pos.num_policies, 0);
}

#[test]
fn test_add_authority_success_secp256k1_with_policy() {
    let mut env = setup_env();
    let wallet_id = [22u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    let secp_key = [7u8; 33];

    // Register
    let reg_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let reg_builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(env.sol_limit_id_pubkey);
        futures::executor::block_on(reg_builder.build_transaction(&conn)).unwrap()
    };
    let mut signed_reg_tx = Transaction::new_unsigned(reg_tx.message);
    signed_reg_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_tx)).unwrap();

    let policy_bytes = PolicyConfigBuilder::new()
        .add_policy(env.sol_limit_id_pubkey, vec![0u8; 8])
        .build();

    let (registry_pda, _) = Pubkey::find_program_address(
        &[
            lazorkit_sdk::state::PolicyRegistryEntry::SEED_PREFIX,
            &env.sol_limit_id_pubkey.to_bytes(),
        ],
        &env.program_id,
    );

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(secp_key.to_vec())
        .with_type(AuthorityType::Secp256k1)
        .with_policy_config(policy_bytes)
        .with_authorization_data(vec![3])
        .with_authorizer(owner_kp.pubkey())
        .with_registry(registry_pda);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    let acc = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = acc.data;
    let wallet_header =
        unsafe { LazorKitWallet::load_unchecked(&data[0..LazorKitWallet::LEN]).unwrap() };

    let pos0 = unsafe { Position::load_unchecked(&data[LazorKitWallet::LEN..]).unwrap() };
    let role1_pos = unsafe { Position::load_unchecked(&data[pos0.boundary as usize..]).unwrap() };
    assert_eq!(role1_pos.authority_type, AuthorityType::Secp256k1 as u16);
}

#[test]
fn test_add_authority_fail_unauthorized_signer() {
    let mut env = setup_env();
    let wallet_id = [23u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    let other_kp = Keypair::new();

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(Keypair::new().pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_authorization_data(vec![3])
        .with_authorizer(other_kp.pubkey()); // Signed by other_kp

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &other_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    assert!(res.is_err());
}
