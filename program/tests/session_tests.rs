mod common;

use common::*;
use lazorkit_program::compact::{self, CompactInstruction};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};

#[test]
fn test_session_lifecycle() {
    let mut context = setup_test();

    // 1. Create Wallet (Owner)
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    let (wallet_pda, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    let (owner_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            owner_keypair.pubkey().as_ref(),
        ],
        &context.program_id,
    );

    // Create Wallet logic
    {
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&user_seed);
        instruction_data.push(0); // Ed25519
        instruction_data.push(0); // Owner role
        instruction_data.extend_from_slice(&[0; 6]); // padding
        instruction_data.extend_from_slice(owner_keypair.pubkey().as_ref());

        let create_wallet_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(owner_auth_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: {
                let mut data = vec![0]; // CreateWallet discriminator
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
            .expect("CreateWallet failed");
    }

    // Fund Vault
    {
        let transfer_ix = solana_sdk::system_instruction::transfer(
            &context.payer.pubkey(),
            &vault_pda,
            1_000_000,
        );
        let message = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[transfer_ix],
            &[],
            context.svm.latest_blockhash(),
        )
        .unwrap();
        let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&context.payer])
            .unwrap();
        context.svm.send_transaction(tx).expect("Fund vault failed");
    }

    // 2. Create Session
    let session_keypair = Keypair::new();
    let current_slot = context.svm.get_sysvar::<solana_sdk::clock::Clock>().slot;
    let expires_at = current_slot + 100; // Expires in 100 slots

    let (session_pda, _) = Pubkey::find_program_address(
        &[
            b"session",
            wallet_pda.as_ref(),
            session_keypair.pubkey().as_ref(),
        ],
        &context.program_id,
    );

    {
        // session_key(32) + expires_at(8)
        let mut session_args = Vec::new();
        session_args.extend_from_slice(session_keypair.pubkey().as_ref());
        session_args.extend_from_slice(&expires_at.to_le_bytes());

        let create_session_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(owner_auth_pda, false), // Authorizer
                AccountMeta::new(session_pda, false),    // New Session PDA
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(owner_keypair.pubkey(), true), // Signer
            ],
            data: {
                let mut data = vec![5]; // CreateSession discriminator
                data.extend_from_slice(&session_args);
                data
            },
        };

        let message = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[create_session_ix],
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
            .expect("CreateSession failed");
    }
    println!("✅ Session created");

    // 3. Execute with Session (Success)
    {
        let transfer_amount = 1000u64;
        let mut transfer_data = Vec::new();
        transfer_data.extend_from_slice(&2u32.to_le_bytes()); // System Transfer
        transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());

        let compact_ix = CompactInstruction {
            program_id_index: 2,
            accounts: vec![0, 1],
            account_roles: vec![3, 1], // Vault: Signer+Writable, Payer: Writable
            data: transfer_data,
        };
        let compact_bytes = compact::serialize_compact_instructions(&[compact_ix]);

        let execute_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(session_pda, false), // Session PDA as Authority
                AccountMeta::new(vault_pda, false),
                // Inner accounts
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(context.payer.pubkey(), false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                // Signer for Session Match
                AccountMeta::new_readonly(session_keypair.pubkey(), true),
            ],
            data: {
                let mut data = vec![4]; // Execute discriminator
                data.extend_from_slice(&compact_bytes);
                data
            },
        };

        let message = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[execute_ix],
            &[],
            context.svm.latest_blockhash(),
        )
        .unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            &[&context.payer, &session_keypair], // Session Key signs
        )
        .unwrap();
        context
            .svm
            .send_transaction(tx)
            .expect("Session execution failed");
    }
    println!("✅ Session execution succeeded");

    // 4. Execute with Expired Session (Fail)
    {
        // Warp time
        let mut clock = context.svm.get_sysvar::<solana_sdk::clock::Clock>();
        clock.slot = expires_at + 1;
        context.svm.set_sysvar(&clock);

        let transfer_amount = 1000u64;
        let mut transfer_data = Vec::new();
        transfer_data.extend_from_slice(&2u32.to_le_bytes());
        transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());
        let compact_ix = CompactInstruction {
            program_id_index: 2,
            accounts: vec![0, 1],
            account_roles: vec![3, 1],
            data: transfer_data,
        };
        let compact_bytes = compact::serialize_compact_instructions(&[compact_ix]);

        let execute_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(session_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(context.payer.pubkey(), false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(session_keypair.pubkey(), true),
            ],
            data: {
                let mut data = vec![4];
                data.extend_from_slice(&compact_bytes);
                data
            },
        };

        let message = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[execute_ix],
            &[],
            context.svm.latest_blockhash(),
        )
        .unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            &[&context.payer, &session_keypair],
        )
        .unwrap();

        let res = context.svm.send_transaction(tx);
        assert!(res.is_err());
        // Could verify specific error but SessionExpired is expected
    }
    println!("✅ Expired session rejected");
}
