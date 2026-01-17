mod common;
use common::{bridge_tx, setup_env, to_sdk_hash, LiteSVMConnection};
use lazorkit_sdk::basic::actions::{AddAuthorityBuilder, CreateWalletBuilder, ExecuteBuilder};
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_state::authority::AuthorityType;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

#[test]
fn test_sdk_usage_high_level() {
    let mut env = setup_env();

    // 1. Create Wallet
    let owner_kp = Keypair::new();
    let wallet_id = [1u8; 32];

    let create_builder = CreateWalletBuilder::new()
        .with_id(wallet_id)
        .with_payer(env.payer.pubkey())
        .with_owner_authority_type(AuthorityType::Ed25519)
        .with_owner_authority_key(owner_kp.pubkey().to_bytes().to_vec());

    let create_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(create_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_create_tx = Transaction::new_unsigned(create_tx.message);
    signed_create_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm
        .send_transaction(bridge_tx(signed_create_tx))
        .unwrap();

    let (config_pda, vault_pda) = create_builder.get_pdas();
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Add Authority
    let new_auth_kp = Keypair::new();
    let add_builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(new_auth_kp.pubkey().to_bytes().to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_authorization_data(vec![3])
        .with_authorizer(owner_kp.pubkey());

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

    // 3. Execute
    let recipient = Keypair::new().pubkey();
    let target_ix = solana_sdk::system_instruction::transfer(&vault_pda, &recipient, 1000);

    // Fund vault
    env.svm
        .airdrop(
            &solana_address::Address::from(vault_pda.to_bytes()),
            1_000_000_000,
        )
        .unwrap();

    let exec_builder = ExecuteBuilder::new(&wallet)
        .with_acting_role(0)
        .add_instruction(target_ix)
        .with_signer(owner_kp.pubkey());

    let exec_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(exec_builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };
    let mut signed_exec_tx = Transaction::new_unsigned(exec_tx.message);
    signed_exec_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_exec_tx)).unwrap();

    // Verify recipient balance
    let recipient_acc = env
        .svm
        .get_account(&solana_address::Address::from(recipient.to_bytes()))
        .unwrap();
    assert_eq!(recipient_acc.lamports, 1000);
}
