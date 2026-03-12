mod common;

use common::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};

#[test]
fn test_close_wallet_lifecycle() {
    let mut context = setup_test();

    // 1. Create Wallet
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();
    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) = Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    let (owner_auth_pda, owner_bump) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), owner_keypair.pubkey().as_ref()],
        &context.program_id,
    );

    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], &context.program_id);
    let (treasury_pda, _) = Pubkey::find_program_address(&[b"treasury", &[0]], &context.program_id);

    {
        let mut create_data = vec![0]; // CreateWallet discriminator
        create_data.extend_from_slice(&user_seed); // 32 bytes
        create_data.push(0); // auth_type = Ed25519 (byte 33)
        create_data.push(owner_bump); // auth_bump (byte 34)
        create_data.extend_from_slice(&[0; 6]); // padding (bytes 35-40)
        create_data.extend_from_slice(owner_keypair.pubkey().as_ref()); // id_seed (32 bytes)

        let create_wallet_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(owner_auth_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new(config_pda, false),
                AccountMeta::new(treasury_pda, false),
            ],
            data: create_data,
        };

        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(v0::Message::try_compile(
                &context.payer.pubkey(),
                &[create_wallet_ix],
                &[],
                context.svm.latest_blockhash(),
            ).unwrap()),
            &[&context.payer],
        ).unwrap();
        context.svm.send_transaction(tx).expect("CreateWallet failed");
    }

    // 2. Close Wallet
    let destination = Keypair::new();
    
    let close_wallet_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new_readonly(owner_auth_pda, false),
            AccountMeta::new(destination.pubkey(), false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: vec![9], // CloseWallet
    };

    let tx_close = VersionedTransaction::try_new(
        VersionedMessage::V0(v0::Message::try_compile(
            &context.payer.pubkey(),
            &[close_wallet_ix],
            &[],
            context.svm.latest_blockhash(),
        ).unwrap()),
        &[&context.payer, &owner_keypair],
    ).unwrap();

    context.svm.send_transaction(tx_close).expect("CloseWallet failed");
    
    // Verify accounts closed
    assert!(context.svm.get_account(&wallet_pda).is_none() || context.svm.get_account(&wallet_pda).unwrap().lamports == 0);
    assert!(context.svm.get_account(&vault_pda).is_none() || context.svm.get_account(&vault_pda).unwrap().lamports == 0);
    println!("✅ Wallet and Vault closed successfully");
}

#[test]
fn test_close_session_lifecycle() {
    let mut context = setup_test();

    // 1. Create Wallet
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();
    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) = Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    let (owner_auth_pda, owner_bump) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), owner_keypair.pubkey().as_ref()],
        &context.program_id,
    );
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], &context.program_id);
    let (treasury_pda, _) = Pubkey::find_program_address(&[b"treasury", &[0]], &context.program_id);

    // Create Wallet
    {
        let mut create_data = vec![0];
        create_data.extend_from_slice(&user_seed);
        create_data.push(0); // Ed25519
        create_data.push(owner_bump);
        create_data.extend_from_slice(&[0; 6]);
        create_data.extend_from_slice(owner_keypair.pubkey().as_ref());
        
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(v0::Message::try_compile(
                &context.payer.pubkey(),
                &[Instruction {
                    program_id: context.program_id,
                    accounts: vec![
                        AccountMeta::new(context.payer.pubkey(), true),
                        AccountMeta::new(wallet_pda, false),
                        AccountMeta::new(vault_pda, false),
                        AccountMeta::new(owner_auth_pda, false),
                        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                        AccountMeta::new(config_pda, false),
                        AccountMeta::new(treasury_pda, false),
                    ],
                    data: create_data,
                }],
                &[],
                context.svm.latest_blockhash(),
            ).unwrap()),
            &[&context.payer],
        ).unwrap();
        context.svm.send_transaction(tx).expect("CreateWallet failed");
    }

    // 2. Create Session
    let session_key = Keypair::new();
    let (session_pda, _) = Pubkey::find_program_address(
        &[b"session", wallet_pda.as_ref(), session_key.pubkey().as_ref()],
        &context.program_id,
    );

    {
        let mut session_data = vec![5]; // CreateSession
        session_data.extend_from_slice(session_key.pubkey().as_ref());
        session_data.extend_from_slice(&2000000000u64.to_le_bytes()); // far future expiration

        let create_session_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new_readonly(wallet_pda, false),
                AccountMeta::new_readonly(owner_auth_pda, false),
                AccountMeta::new(session_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new_readonly(config_pda, false),
                AccountMeta::new(treasury_pda, false),
                AccountMeta::new_readonly(owner_keypair.pubkey(), true),
            ],
            data: session_data,
        };

        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(v0::Message::try_compile(
                &context.payer.pubkey(),
                &[create_session_ix],
                &[],
                context.svm.latest_blockhash(),
            ).unwrap()),
            &[&context.payer, &owner_keypair],
        ).unwrap();
        context.svm.send_transaction(tx).expect("CreateSession failed");
    }

    // 3. Close Session
    {
        let close_session_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true), // receives refund
                AccountMeta::new_readonly(wallet_pda, false),
                AccountMeta::new(session_pda, false),
                AccountMeta::new_readonly(config_pda, false),
                AccountMeta::new_readonly(owner_auth_pda, false), // optional but used for owner check
                AccountMeta::new_readonly(owner_keypair.pubkey(), true),
            ],
            data: vec![8], // CloseSession
        };

        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(v0::Message::try_compile(
                &context.payer.pubkey(),
                &[close_session_ix],
                &[],
                context.svm.latest_blockhash(),
            ).unwrap()),
            &[&context.payer, &owner_keypair],
        ).unwrap();
        context.svm.send_transaction(tx).expect("CloseSession failed");
    }

    // Verify session closed
    assert!(context.svm.get_account(&session_pda).is_none() || context.svm.get_account(&session_pda).unwrap().lamports == 0);
    println!("✅ Session closed successfully");
}
