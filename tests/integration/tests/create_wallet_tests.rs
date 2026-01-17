mod common;
use common::{bridge_tx, setup_env, to_sdk_hash, LiteSVMConnection};
use lazorkit_sdk::advanced::types::Transmutable;
use lazorkit_sdk::basic::actions::CreateWalletBuilder;
use lazorkit_sdk::state::{AuthorityType, LazorKitWallet, Position};
use solana_address::Address;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};

#[test]
fn test_create_wallet_success() {
    let mut env = setup_env();
    let wallet_id = [7u8; 32];
    let owner_kp = Keypair::new();

    let builder = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Ed25519)
        .with_owner_authority_key(owner_kp.pubkey().to_bytes().to_vec());

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    let (config_pda, _) = builder.get_pdas();

    // Verify Config
    let config_account = env
        .svm
        .get_account(&Address::from(config_pda.to_bytes()))
        .expect("Config account not found");
    let data = config_account.data;
    let wallet_header =
        unsafe { LazorKitWallet::load_unchecked(&data[0..LazorKitWallet::LEN]).unwrap() };

    assert_eq!(wallet_header.role_count, 1);
    assert_eq!(wallet_header.role_counter, 1);

    let pos_data = &data[LazorKitWallet::LEN..];
    let pos = unsafe { Position::load_unchecked(pos_data).unwrap() };
    assert_eq!(pos.authority_type, AuthorityType::Ed25519 as u16);
    assert_eq!(pos.id, 0);
}

#[test]
fn test_create_wallet_with_secp256k1_authority() {
    let mut env = setup_env();
    let wallet_id = [9u8; 32];
    let fake_secp_key = [1u8; 33]; // Fixed to 33 bytes for compressed public key

    let builder = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Secp256k1)
        .with_owner_authority_key(fake_secp_key.to_vec());

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection)).unwrap()
    };
    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    let (config_pda, _) = builder.get_pdas();
    let config_account = env
        .svm
        .get_account(&Address::from(config_pda.to_bytes()))
        .expect("Config account not found");
    let data = config_account.data;
    let pos = unsafe { Position::load_unchecked(&data[LazorKitWallet::LEN..]).unwrap() };

    assert_eq!(pos.authority_type, AuthorityType::Secp256k1 as u16);
    assert_eq!(pos.authority_length, 40);
}

#[test]
fn test_create_wallet_fail_invalid_seeds() {
    let mut env = setup_env();
    let wallet_id = [8u8; 32];
    let owner_kp = Keypair::new();

    let builder = CreateWalletBuilder::new()
        .with_id(wallet_id)
        .with_payer(env.payer.pubkey())
        .with_owner_authority_type(AuthorityType::Ed25519)
        .with_owner_authority_key(owner_kp.pubkey().to_bytes().to_vec());

    let mut tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection)).unwrap()
    };

    // Case 1: Wrong Config Account
    let config_idx = tx.message.instructions[0].accounts[0] as usize;
    let original_config = tx.message.account_keys[config_idx];
    tx.message.account_keys[config_idx] = Keypair::new().pubkey();

    let mut signed_tx = Transaction::new_unsigned(tx.message.clone());
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    assert!(env.svm.send_transaction(bridge_tx(signed_tx)).is_err());

    // Case 2: Wrong Vault Account
    tx.message.account_keys[config_idx] = original_config; // Restore
    let vault_idx = tx.message.instructions[0].accounts[2] as usize;
    tx.message.account_keys[vault_idx] = Keypair::new().pubkey();

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    assert!(env.svm.send_transaction(bridge_tx(signed_tx)).is_err());
}
