use common::*;
use lazorkit_sdk::basic::actions::*;
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::AuthorityType;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

mod common;

// No local helper needed, using lazorkit_sdk::basic::actions::create_secp256r1_session_data

// Create wallet with Secp256r1Session authority
pub fn create_wallet_with_secp256r1_session(
    env: &mut TestEnv,
    wallet_id: [u8; 32],
    owner_pubkey: [u8; 33],
) -> (Pubkey, Pubkey) {
    let connection = LiteSVMConnection { svm: &env.svm };
    let owner_data = create_secp256r1_session_data(owner_pubkey, 1000);

    let builder = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Secp256r1Session)
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
fn test_create_secp256r1_session_wallet() {
    let mut env = setup_env();
    let wallet_id = [5u8; 32];

    // Use a mock Secp256r1 compressed public key (33 bytes)
    // Secp256r1 (P-256) used in WebAuthn/Passkeys
    let owner_pubkey = [3u8; 33]; // Compressed pubkey starts with 0x02 or 0x03

    let (config_pda, vault_pda) =
        create_wallet_with_secp256r1_session(&mut env, wallet_id, owner_pubkey);

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
fn test_create_session_with_secp256r1() {
    let mut env = setup_env();
    let wallet_id = [6u8; 32];
    let owner_pubkey = [3u8; 33];

    // 1. Create Wallet with Secp256r1Session authority
    let (config_pda, vault_pda) =
        create_wallet_with_secp256r1_session(&mut env, wallet_id, owner_pubkey);

    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Create Session Key
    let session_key = Keypair::new();
    let duration = 100;

    // Note: In real scenario, owner would be a Secp256r1 keypair (WebAuthn)
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
    // because mock_signer is not the actual Secp256r1 owner
    // This test demonstrates the SDK can construct the transaction correctly
    let res = env.svm.send_transaction(bridge_tx(signed_tx));

    // Expected to fail due to signature mismatch, but transaction should be well-formed
    assert!(res.is_err(), "Should fail due to signature verification");
}

#[test]
fn test_add_secp256r1_session_authority() {
    let mut env = setup_env();
    let owner = Keypair::new();
    let wallet_id = [7u8; 32];

    // 1. Create wallet with Ed25519 owner first
    let (config_pda, vault_pda) =
        common::create_wallet(&mut env, wallet_id, &owner, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Add Secp256r1Session authority (for WebAuthn/Passkey support)
    let secp256r1_pubkey = [3u8; 33];
    let session_data = create_secp256r1_session_data(secp256r1_pubkey, 2000);

    let builder = AddAuthorityBuilder::new(&wallet)
        .with_authority_key(session_data)
        .with_type(AuthorityType::Secp256r1Session)
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
fn test_secp256r1_session_data_validation() {
    let env = setup_env();
    let wallet_id = [8u8; 32];
    let _owner_pubkey = [3u8; 33];

    // For Secp256r1Session, data should be 80 bytes (Create data)
    let invalid_data_72_bytes = vec![0u8; 72]; // Wrong size

    let connection = LiteSVMConnection { svm: &env.svm };
    let builder_invalid = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Secp256r1Session)
        .with_owner_authority_key(invalid_data_72_bytes);

    let result_invalid =
        futures::executor::block_on(builder_invalid.build_transaction(&connection));

    assert!(
        result_invalid.is_err(),
        "Should reject invalid data length (72 bytes)"
    );
    assert!(
        result_invalid
            .unwrap_err()
            .contains("Invalid Secp256r1Session data length"),
        "Error should mention invalid length"
    );

    // Test with correct data length (80 bytes)
    let correct_data = vec![0u8; 80];
    let builder_correct = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Secp256r1Session)
        .with_owner_authority_key(correct_data);

    let result_correct =
        futures::executor::block_on(builder_correct.build_transaction(&connection));
    assert!(
        result_correct.is_ok(),
        "Should accept correct data length (80 bytes)"
    );
}

#[test]
fn test_secp256r1_passkey_use_case() {
    let mut env = setup_env();
    let wallet_id = [9u8; 32];

    // Simulate a WebAuthn/Passkey public key
    // In real scenario, this would come from navigator.credentials.create()
    let passkey_pubkey = [2u8; 33]; // P-256 compressed public key
    let _max_session_age = 3600; // 1 hour in slots (~30 min in real time)

    let (config_pda, _vault_pda) =
        create_wallet_with_secp256r1_session(&mut env, wallet_id, passkey_pubkey);

    // Verify wallet creation
    let config_account = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .expect("Passkey wallet should be created");

    assert!(config_account.lamports > 0);

    // This demonstrates that wallets can be created with passkey authentication
    // enabling passwordless, phishing-resistant authentication for Solana wallets
}

#[test]
fn test_helper_function_creates_correct_data() {
    let pubkey = [2u8; 33];
    let max_age = 5000u64;

    let data = create_secp256r1_session_data(pubkey, max_age);

    // Verify structure (CreateSecp256r1SessionAuthority: 80 bytes)
    assert_eq!(data.len(), 80, "Should be 80 bytes for initialization");
    assert_eq!(&data[0..33], &pubkey, "First 33 bytes should be pubkey");
    assert_eq!(&data[33..40], &[0u8; 7], "Bytes 33-39 should be padding");
    assert_eq!(
        &data[40..72],
        &[0u8; 32],
        "Bytes 40-71 should be empty session_key"
    );
    assert_eq!(
        &data[72..80],
        &max_age.to_le_bytes(),
        "Bytes 72-79 should be max_session_length"
    );
}
