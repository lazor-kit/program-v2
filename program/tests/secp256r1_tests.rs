mod common;

use common::*;
use p256::ecdsa::{SigningKey, VerifyingKey};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signer::Signer,
    transaction::VersionedTransaction,
};

#[test]
fn test_create_wallet_secp256r1_repro() {
    let mut context = setup_test();

    // 1. Generate Secp256r1 Key
    let mut rng = rand::thread_rng();
    let signing_key = SigningKey::random(&mut rng);
    let verifying_key = VerifyingKey::from(&signing_key);
    let pubkey_bytes = verifying_key.to_encoded_point(true).as_bytes().to_vec(); // 33 bytes compressed

    // Fake credential ID hash (used as PDA seed)
    let credential_id_hash = [2u8; 32];

    let user_seed = rand::random::<[u8; 32]>();

    let (wallet_pda, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    // Authority seed for Secp256r1 is the credential_id_hash
    let (auth_pda, auth_bump) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &credential_id_hash],
        &context.program_id,
    );

    // Build CreateWallet args
    // [user_seed(32)][type(1)][bump(1)][padding(6)][credential_id_hash(32)][pubkey(33)]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&user_seed);
    instruction_data.push(1); // Secp256r1
    instruction_data.push(auth_bump); // Use correct bump
    instruction_data.extend_from_slice(&[0; 6]); // padding

    // "rest" part for Secp256r1: credential_id_hash + pubkey
    instruction_data.extend_from_slice(&credential_id_hash);
    instruction_data.extend_from_slice(&pubkey_bytes);

    // Derive Config + Treasury shard used by CreateWallet fee logic
    let (config_pda, _) =
        Pubkey::find_program_address(&[b"config"], &context.program_id);
    let shard_id: u8 = 0;
    let shard_id_bytes = [shard_id];
    let (treasury_pda, _) =
        Pubkey::find_program_address(&[b"treasury", &shard_id_bytes], &context.program_id);

    let create_wallet_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(auth_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            AccountMeta::new(config_pda, false),
            AccountMeta::new(treasury_pda, false),
        ],
        data: {
            let mut data = vec![0]; // Discriminator
            data.extend_from_slice(&instruction_data);
            data
        },
    };

    let message = v0::Message::try_compile(
        &context.payer.pubkey(),
        &[create_wallet_ix],
        &[],
        context.svm.latest_blockhash(),
    )
    .unwrap();

    let tx =
        VersionedTransaction::try_new(VersionedMessage::V0(message), &[&context.payer]).unwrap();

    // This should now SUCCEED
    context
        .svm
        .send_transaction(tx)
        .expect("CreateWallet Secp256r1 failed");
    println!("✅ Wallet created with Secp256r1 Authority");
}

#[test]
fn test_add_multiple_secp256r1_authorities() {
    let mut context = setup_test();

    // 1. Setup Wallet with First Ed25519 Authority (Owner)
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = solana_sdk::signature::Keypair::new();
    let owner_pubkey = owner_keypair.pubkey();

    let (wallet_pda, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    let (owner_pda, owner_bump) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), owner_pubkey.as_ref()],
        &context.program_id,
    );

    {
        // Derive Config + Treasury shard for this wallet
        let (config_pda, _) =
            Pubkey::find_program_address(&[b"config"], &context.program_id);
        let shard_id: u8 = 0;
        let shard_id_bytes = [shard_id];
        let (treasury_pda, _) =
            Pubkey::find_program_address(&[b"treasury", &shard_id_bytes], &context.program_id);
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&user_seed);
        instruction_data.push(0); // Ed25519
        instruction_data.push(owner_bump);
        instruction_data.extend_from_slice(&[0; 6]); // padding
        instruction_data.extend_from_slice(owner_pubkey.as_ref());

        let create_wallet_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(owner_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new(config_pda, false),
                AccountMeta::new(treasury_pda, false),
            ],
            data: {
                let mut data = vec![0]; // CreateWallet
                data.extend_from_slice(&instruction_data);
                data
            },
        };

        let message = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[create_wallet_ix],
            &[],
            context.svm.latest_blockhash(),
        )
        .unwrap();
        let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&context.payer])
            .unwrap();
        context
            .svm
            .send_transaction(tx)
            .expect("CreateWallet Ed25519 failed");
    }

    // Passkeys from the SAME domain
    let mut rng = rand::thread_rng();

    // --- Add Passkey 1 ---
    let credential_id_hash1 = [3u8; 32];
    let signing_key1 = SigningKey::random(&mut rng);
    let pubkey_bytes1 = VerifyingKey::from(&signing_key1)
        .to_encoded_point(true)
        .as_bytes()
        .to_vec();
    let (auth_pda1, _auth_bump1) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &credential_id_hash1],
        &context.program_id,
    );

    let add_auth_args = {
        let mut args = Vec::new();
        args.push(1); // Secp256r1 (authority_type)
        args.push(0); // Owner (new_role)
        args.extend_from_slice(&[0; 6]); // padding
        args
    };

    let data_payload = {
        let mut payload = Vec::new();
        payload.extend_from_slice(&add_auth_args);
        payload.extend_from_slice(&credential_id_hash1);
        payload.extend_from_slice(&pubkey_bytes1);
        payload
    };

    let signature = owner_keypair.sign_message(&data_payload);
    let mut add_auth_ix_data = vec![1]; // AddAuthority (discriminator 1)
    add_auth_ix_data.extend_from_slice(&add_auth_args);
    add_auth_ix_data.extend_from_slice(&credential_id_hash1);
    add_auth_ix_data.extend_from_slice(&pubkey_bytes1);
    add_auth_ix_data.extend_from_slice(signature.as_ref());

    // Re-derive Config + Treasury shard (same as used for wallet creation)
    let (config_pda, _) =
        Pubkey::find_program_address(&[b"config"], &context.program_id);
    let shard_id: u8 = 0;
    let shard_id_bytes = [shard_id];
    let (treasury_pda, _) =
        Pubkey::find_program_address(&[b"treasury", &shard_id_bytes], &context.program_id);

    let add_auth_ix1 = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(owner_pda, false), // Admin Auth PDA (Ed25519) - Writable for counter
            AccountMeta::new(auth_pda1, false), // New Auth PDA (Secp256r1)
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            // Config + Treasury shard for protocol fee
            AccountMeta::new(config_pda, false),
            AccountMeta::new(treasury_pda, false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true), // Ed25519 Master Signer (after positional accounts)
        ],
        data: add_auth_ix_data,
    };

    let message = v0::Message::try_compile(
        &context.payer.pubkey(),
        &[add_auth_ix1],
        &[],
        context.svm.latest_blockhash(),
    )
    .unwrap();

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[&context.payer, &owner_keypair],
    )
    .unwrap();

    context
        .svm
        .send_transaction(tx)
        .expect("AddAuthority Secp256r1 (1) failed");
    println!("✅ Added Secp256r1 Authority 1");

    // Each passkey has a unique credential_id, producing a unique credential_id_hash.
    // This means each passkey derives a unique Authority PDA, preventing collisions.
    // --- Add Passkey 2 (Same Domain, Different credential_id) ---
    let credential_id_hash2 = [4u8; 32];
    let signing_key2 = SigningKey::random(&mut rng);
    let pubkey_bytes2 = VerifyingKey::from(&signing_key2)
        .to_encoded_point(true)
        .as_bytes()
        .to_vec();
    let (auth_pda2, _auth_bump2) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &credential_id_hash2],
        &context.program_id,
    );

    let data_payload = {
        let mut payload = Vec::new();
        payload.extend_from_slice(&add_auth_args);
        payload.extend_from_slice(&credential_id_hash2);
        payload.extend_from_slice(&pubkey_bytes2);
        payload
    };

    let signature = owner_keypair.sign_message(&data_payload);
    let mut add_auth_ix_data = vec![1]; // AddAuthority (discriminator 1)
    add_auth_ix_data.extend_from_slice(&add_auth_args);
    add_auth_ix_data.extend_from_slice(&credential_id_hash2);
    add_auth_ix_data.extend_from_slice(&pubkey_bytes2);
    add_auth_ix_data.extend_from_slice(signature.as_ref());

    let add_auth_ix2 = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(owner_pda, false), // Admin Auth PDA (Ed25519) - Writable for counter
            AccountMeta::new(auth_pda2, false), // New Auth PDA (Secp256r1)
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            // Config + Treasury shard for protocol fee
            AccountMeta::new(config_pda, false),
            AccountMeta::new(treasury_pda, false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true), // Ed25519 Master Signer (after positional accounts)
        ],
        data: add_auth_ix_data,
    };

    let message = v0::Message::try_compile(
        &context.payer.pubkey(),
        &[add_auth_ix2],
        &[],
        context.svm.latest_blockhash(),
    )
    .unwrap();

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[&context.payer, &owner_keypair],
    )
    .unwrap();

    context
        .svm
        .send_transaction(tx)
        .expect("AddAuthority Secp256r1 (2) failed");
    println!("✅ Added Secp256r1 Authority 2");

    assert_ne!(
        pubkey_bytes1[1..33],
        pubkey_bytes2[1..33],
        "Passkey X-coordinates must be unique"
    );
    assert_ne!(
        auth_pda1, auth_pda2,
        "Authority PDAs must not collide for passkeys on the same domain"
    );

    println!("✅ Passed: Multiple Secp256r1 Authorities from identical domain derive unique PDAs successfully");
}
