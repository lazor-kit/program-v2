mod common;
use common::{bridge_tx, create_wallet, setup_env, to_sdk_hash, LiteSVMConnection};
use lazorkit_sdk::advanced::types::Transmutable;
use lazorkit_sdk::basic::actions::{
    AddAuthorityBuilder, RegisterPolicyBuilder, RemoveAuthorityBuilder, UpdateAuthorityBuilder,
};
use lazorkit_sdk::basic::policy::PolicyConfigBuilder;
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::{AuthorityType, LazorKitWallet};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

#[test]
fn test_add_multiple_authorities_and_verify_state() {
    let mut env = setup_env();
    let wallet_id = [100u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // Initial state: 1 authority (owner)
    let acc = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    let initial_wallet =
        unsafe { LazorKitWallet::load_unchecked(&acc.data[0..LazorKitWallet::LEN]).unwrap() };
    assert_eq!(initial_wallet.role_count, 1);

    // Add 4 more authorities (total 5)
    for i in 1..5 {
        let new_kp = Keypair::new();
        let tx = {
            let conn = LiteSVMConnection { svm: &env.svm };
            let builder = AddAuthorityBuilder::new(&wallet)
                .with_authority(new_kp.pubkey())
                .with_type(AuthorityType::Ed25519)
                .with_authorizer(owner_kp.pubkey())
                .with_authorization_data(vec![3]);
            futures::executor::block_on(builder.build_transaction(&conn, env.payer.pubkey()))
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

        let current_acc = env
            .svm
            .get_account(&solana_address::Address::from(config_pda.to_bytes()))
            .unwrap();
        let current_wallet = unsafe {
            LazorKitWallet::load_unchecked(&current_acc.data[0..LazorKitWallet::LEN]).unwrap()
        };
        assert_eq!(current_wallet.role_count, (i + 1) as u16);
    }
}

#[test]
fn test_remove_authority_success() {
    let mut env = setup_env();
    let wallet_id = [101u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 1. Add a second authority
    let other_kp = Keypair::new();
    let add_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let add_builder = AddAuthorityBuilder::new(&wallet)
            .with_authority(other_kp.pubkey())
            .with_type(AuthorityType::Ed25519)
            .with_authorizer(owner_kp.pubkey())
            .with_authorization_data(vec![3]);
        futures::executor::block_on(add_builder.build_transaction(&conn, env.payer.pubkey()))
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

    let acc_after_add = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    let wallet_after_add = unsafe {
        LazorKitWallet::load_unchecked(&acc_after_add.data[0..LazorKitWallet::LEN]).unwrap()
    };
    assert_eq!(wallet_after_add.role_count, 2);

    // 2. Remove the second authority (Role ID 1)
    let remove_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let remove_builder = RemoveAuthorityBuilder::new(&wallet)
            .with_acting_role(0) // Owner
            .with_target_role(1)
            .with_authorizer(owner_kp.pubkey());
        futures::executor::block_on(remove_builder.build_transaction(&conn, env.payer.pubkey()))
            .unwrap()
    };

    let mut signed_remove_tx = Transaction::new_unsigned(remove_tx.message);
    signed_remove_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm
        .send_transaction(bridge_tx(signed_remove_tx))
        .unwrap();

    let acc_after_remove = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    let wallet_after_remove = unsafe {
        LazorKitWallet::load_unchecked(&acc_after_remove.data[0..LazorKitWallet::LEN]).unwrap()
    };
    assert_eq!(wallet_after_remove.role_count, 1);
}

#[test]
fn test_update_authority_replace_policies() {
    let mut env = setup_env();
    let wallet_id = [102u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 0. Register Policy
    let reg_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(env.sol_limit_id_pubkey);
        futures::executor::block_on(builder.build_transaction(&conn)).unwrap()
    };
    let mut signed_reg_tx = Transaction::new_unsigned(reg_tx.message);
    signed_reg_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_tx)).unwrap();

    // 1. Add authority with NO policies
    let other_kp = Keypair::new();
    let add_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let add_builder = AddAuthorityBuilder::new(&wallet)
            .with_authority(other_kp.pubkey())
            .with_type(AuthorityType::Ed25519)
            .with_authorizer(owner_kp.pubkey())
            .with_authorization_data(vec![3]);
        futures::executor::block_on(add_builder.build_transaction(&conn, env.payer.pubkey()))
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

    // 2. Update authority with a policy (ReplaceAll)
    let policy_bytes = PolicyConfigBuilder::new()
        .add_policy(env.sol_limit_id_pubkey, vec![50u8; 8]) // 50 lamports limit
        .build();

    let mut payload = (1u32).to_le_bytes().to_vec();
    payload.extend(policy_bytes);

    let update_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let update_builder = UpdateAuthorityBuilder::new(&wallet)
            .with_acting_role(0)
            .with_target_role(1)
            .with_operation(0) // ReplaceAll
            .with_payload(payload)
            .with_registry(
                Pubkey::find_program_address(
                    &[
                        b"policy-registry",
                        env.sol_limit_id_pubkey.as_ref(), // Policy Program ID
                    ],
                    &env.program_id,
                )
                .0,
            )
            .with_authorizer(owner_kp.pubkey());
        futures::executor::block_on(update_builder.build_transaction(&conn, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_update_tx = Transaction::new_unsigned(update_tx.message);
    signed_update_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm
        .send_transaction(bridge_tx(signed_update_tx))
        .unwrap();

    // Verify state
    let acc = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = acc.data;

    // Position 0 is owner, Position 1 is newly added auth
    let pos0 = unsafe {
        lazorkit_sdk::state::Position::load_unchecked(&data[LazorKitWallet::LEN..]).unwrap()
    };
    let pos1 = unsafe {
        lazorkit_sdk::state::Position::load_unchecked(&data[pos0.boundary as usize..]).unwrap()
    };

    assert_eq!(pos1.num_policies, 1);
}
