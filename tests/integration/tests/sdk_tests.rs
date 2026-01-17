mod common;
use common::{bridge_tx, create_wallet, setup_env, to_sdk_hash, LiteSVMConnection};
use lazorkit_policy_sol_limit::SolLimitState;
use lazorkit_sdk::basic::actions::{AddAuthorityBuilder, ExecuteBuilder, RegisterPolicyBuilder};
use lazorkit_sdk::basic::policy::PolicyConfigBuilder;
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::{AuthorityType, IntoBytes, PolicyRegistryEntry};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

#[test]
fn test_sdk_add_authority_ed25519_with_policy() {
    let mut env = setup_env();
    let wallet_id = [50u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);

    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 1. Register Policy First using SDK
    let reg_builder = RegisterPolicyBuilder::new(env.program_id)
        .with_payer(env.payer.pubkey())
        .with_policy(env.sol_limit_id_pubkey);

    let reg_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(reg_builder.build_transaction(&connection)).unwrap()
    };

    let mut signed_reg_tx = Transaction::new_unsigned(reg_tx.message);
    signed_reg_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_tx)).unwrap();

    // 2. Create Policy Config using PolicyConfigBuilder
    let limit_state = SolLimitState { amount: 500 };
    let policy_bytes = PolicyConfigBuilder::new()
        .add_policy(
            env.sol_limit_id_pubkey,
            limit_state.into_bytes().unwrap().to_vec(),
        )
        .build();

    // 3. Add Authority using SDK
    let new_auth_kp = Keypair::new();
    let (registry_pda, _) = Pubkey::find_program_address(
        &[
            PolicyRegistryEntry::SEED_PREFIX,
            &env.sol_limit_id_pubkey.to_bytes(),
        ],
        &env.program_id,
    );

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(new_auth_kp.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_role(2)
        .with_policy_config(policy_bytes)
        .with_authorization_data(vec![3]) // Index of signer (Owner)
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
}

#[test]
fn test_sdk_add_secp256k1_authority() {
    let mut env = setup_env();
    let wallet_id = [51u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // Mock Secp Key
    let mut secp_key = [0u8; 33];
    secp_key[0] = 0x02;
    secp_key[1] = 0xBB;

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(secp_key.to_vec())
        .with_type(AuthorityType::Secp256k1)
        .with_role(3)
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
}

#[test]
fn test_sdk_execute_transfer() {
    let mut env = setup_env();
    let wallet_id = [52u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // Fund vault
    let transfer_amount = 1_000_000; // 0.001 SOL
    env.svm
        .airdrop(
            &solana_address::Address::from(vault_pda.to_bytes()),
            transfer_amount * 2,
        )
        .unwrap();

    let receiver = Pubkey::new_unique();
    let start_bal = env
        .svm
        .get_balance(&solana_address::Address::from(receiver.to_bytes()))
        .unwrap_or(0);

    // Build target instruction
    let target_ix =
        solana_sdk::system_instruction::transfer(&vault_pda, &receiver, transfer_amount);

    let builder = ExecuteBuilder::new(&wallet)
        .add_instruction(target_ix)
        .with_auth_payload(vec![6]) // Index of Owner in accounts list
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

    let end_bal = env
        .svm
        .get_balance(&solana_address::Address::from(receiver.to_bytes()))
        .unwrap();
    assert_eq!(end_bal, start_bal + transfer_amount);
}
