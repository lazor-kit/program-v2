use lazorkit_sdk::basic::actions::{CreateWalletBuilder, ExecuteBuilder, TransferOwnershipBuilder};
use lazorkit_sdk::basic::wallet::LazorWallet;

use lazorkit_sdk::state::AuthorityType;
use solana_address::Address;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

mod common;
use common::{bridge_tx, setup_env, to_sdk_hash, LiteSVMConnection};

#[test]
fn test_transfer_ownership_success() {
    let mut env = setup_env();
    let owner_kp = Keypair::new();
    let new_owner_kp = Keypair::new();
    let wallet_id = [21u8; 32];

    // Airdrop to owner
    env.svm
        .airdrop(&Address::from(owner_kp.pubkey().to_bytes()), 1_000_000_000)
        .unwrap();

    // 1. Create Wallet
    let create_builder = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_owner(owner_kp.pubkey())
        .with_id(wallet_id);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(create_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    let (config_pda, vault_pda) = create_builder.get_pdas();
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Transfer Ownership to New Owner
    let transfer_builder = TransferOwnershipBuilder::new(&wallet)
        .with_current_owner(owner_kp.pubkey())
        .with_new_owner(
            AuthorityType::Ed25519,
            new_owner_kp.pubkey().to_bytes().to_vec(),
        );

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(transfer_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&owner_kp], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    // 3. Verify Old Owner Cannot Execute
    let execute_builder = ExecuteBuilder::new(&wallet)
        .with_signer(owner_kp.pubkey())
        .add_instruction(solana_sdk::system_instruction::transfer(
            &env.payer.pubkey(), // From payer (just partial signing test)
            &env.payer.pubkey(),
            0,
        ));

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(
            execute_builder.build_transaction(&connection, env.payer.pubkey()),
        )
        .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    // Old owner tries to sign
    signed_tx
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    println!("Step 3 (Old Owner Execution) Result: {:?}", res);
    assert!(res.is_err(), "Old owner should not be able to execute");
    // Optionally check error code to ensure it's PermissionDenied (Custom(4005) or similar)

    // 4. Verify New Owner Can Execute
    let execute_builder = ExecuteBuilder::new(&wallet)
        .with_signer(new_owner_kp.pubkey())
        .add_instruction(solana_sdk::system_instruction::transfer(
            &env.payer.pubkey(),
            &env.payer.pubkey(),
            0,
        ));

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(
            execute_builder.build_transaction(&connection, env.payer.pubkey()),
        )
        .unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &new_owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();
}

#[test]
fn test_transfer_ownership_fail_unauthorized() {
    let mut env = setup_env();
    let owner_kp = Keypair::new();
    let fake_owner_kp = Keypair::new();
    let wallet_id = [22u8; 32];

    // Airdrop to fake owner
    env.svm
        .airdrop(
            &Address::from(fake_owner_kp.pubkey().to_bytes()),
            1_000_000_000,
        )
        .unwrap();

    // 1. Create Wallet
    let create_builder = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_owner(owner_kp.pubkey())
        .with_id(wallet_id);

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(create_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    let (config_pda, vault_pda) = create_builder.get_pdas();
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Try Transfer with Fake Owner
    let transfer_builder = TransferOwnershipBuilder::new(&wallet)
        .with_current_owner(fake_owner_kp.pubkey())
        .with_new_owner(
            AuthorityType::Ed25519,
            Keypair::new().pubkey().to_bytes().to_vec(),
        );

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(transfer_builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    // Fake owner signs
    signed_tx
        .try_sign(&[&fake_owner_kp], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    assert!(res.is_err());
}
