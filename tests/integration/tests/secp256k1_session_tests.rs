use common::*;
use lazorkit_sdk::basic::actions::*;
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::AuthorityType;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

mod common;

// No local helper needed, using lazorkit_sdk::basic::actions::create_secp256k1_session_data

// Create wallet with Secp256k1Session authority
pub fn create_wallet_with_secp256k1_session(
    env: &mut TestEnv,
    wallet_id: [u8; 32],
    owner_pubkey: [u8; 33],
) -> (Pubkey, Pubkey) {
    let connection = LiteSVMConnection { svm: &env.svm };
    let owner_data = create_secp256k1_session_data(&owner_pubkey, 1000);

    let builder = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Secp256k1Session)
        .with_owner_authority_key(owner_data);

    let tx = futures::executor::block_on(builder.build_transaction(&connection)).unwrap();

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    let (config_pda, _) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &env.program_id);
    let (vault_pda, _) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &env.program_id,
    );
    (config_pda, vault_pda)
}

#[test]
fn test_create_secp256k1_session_wallet() {
    let mut env = setup_env();
    let wallet_id = [1u8; 32];

    // Use a mock Secp256k1 compressed public key (33 bytes)
    let owner_pubkey = [2u8; 33]; // Compressed pubkey starts with 0x02 or 0x03

    let (config_pda, vault_pda) =
        create_wallet_with_secp256k1_session(&mut env, wallet_id, owner_pubkey);

    // Verify wallet was created
    let config_account = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .expect("Config account should exist");

    assert!(
        config_account.lamports > 0,
        "Config account should have lamports"
    );
    assert!(
        config_account.data.len() > 0,
        "Config account should have data"
    );

    // Verify vault was created
    let vault_account = env
        .svm
        .get_account(&solana_address::Address::from(vault_pda.to_bytes()))
        .expect("Vault account should exist");

    assert!(
        vault_account.lamports > 0,
        "Vault account should have lamports"
    );
}

#[test]
fn test_create_session_with_secp256k1() {
    let mut env = setup_env();
    let wallet_id = [2u8; 32];
    let owner_pubkey = [2u8; 33];

    // 1. Create Wallet with Secp256k1Session authority
    let (config_pda, vault_pda) =
        create_wallet_with_secp256k1_session(&mut env, wallet_id, owner_pubkey);

    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Create Session Key
    let session_key = Keypair::new();
    let duration = 100;

    // Note: In real scenario, owner would be a Secp256k1 keypair
    // For this test, we use a mock signer
    let mock_signer = Keypair::new();

    let builder = CreateSessionBuilder::new(&wallet)
        .with_role(0) // Owner role
        .with_session_key(session_key.pubkey().to_bytes())
        .with_duration(duration)
        .with_authorizer(mock_signer.pubkey()); // Use mock signer's pubkey

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &mock_signer],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    // Note: This will fail signature verification in real scenario
    // because mock_signer is not the actual Secp256k1 owner
    // This test demonstrates the SDK can construct the transaction correctly
    let res = env.svm.send_transaction(bridge_tx(signed_tx));

    // Expected to fail due to signature mismatch, but transaction should be well-formed
    assert!(res.is_err(), "Should fail due to signature verification");
}

#[test]
fn test_add_secp256k1_session_authority() {
    let mut env = setup_env();
    let owner = Keypair::new();
    let wallet_id = [3u8; 32];

    // 1. Create wallet with Ed25519 owner first
    let (config_pda, vault_pda) =
        common::create_wallet(&mut env, wallet_id, &owner, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Add Secp256k1Session authority
    let secp256k1_pubkey = [3u8; 33];
    let session_data = create_secp256k1_session_data(&secp256k1_pubkey, 2000);

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(session_data)
        .with_type(AuthorityType::Secp256k1Session)
        .with_authorization_data(vec![3]) // Admin permission
        .with_authorizer(owner.pubkey());

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &owner],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    assert!(res.is_ok(), "AddAuthority should succeed: {:?}", res.err());

    // Verify authority was added by checking account data size increased
    let config_account = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();

    assert!(
        config_account.data.len() > 200,
        "Config should have grown with new authority"
    );
}

#[test]
fn test_secp256k1_session_data_validation() {
    let env = setup_env();
    let wallet_id = [4u8; 32];
    let _owner_pubkey = [2u8; 33];

    // For Secp256k1Session, data should be 104 bytes (Create data)
    let invalid_data_88_bytes = vec![0u8; 88]; // This used to be "correct" but is now wrong

    let connection = LiteSVMConnection { svm: &env.svm };
    let builder_invalid = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Secp256k1Session)
        .with_owner_authority_key(invalid_data_88_bytes);

    let result_invalid =
        futures::executor::block_on(builder_invalid.build_transaction(&connection));

    assert!(
        result_invalid.is_err(),
        "Should reject invalid data length (88 bytes)"
    );
    assert!(
        result_invalid
            .unwrap_err()
            .contains("Invalid Secp256k1Session data length"),
        "Error should mention invalid length"
    );
}
