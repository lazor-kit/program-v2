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

/// Helper: create wallet + session, returns all PDAs and keypairs
fn setup_wallet_with_session(
    context: &mut TestContext,
) -> (
    Pubkey,   // wallet_pda
    Pubkey,   // vault_pda
    Pubkey,   // owner_auth_pda
    Keypair,  // owner_keypair
    Pubkey,   // session_pda
    Keypair,  // session_keypair
) {
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    let (wallet_pda, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    let (owner_auth_pda, owner_bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            owner_keypair.pubkey().as_ref(),
        ],
        &context.program_id,
    );

    // Create wallet
    {
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&user_seed);
        instruction_data.push(0); // Ed25519
        instruction_data.push(owner_bump);
        instruction_data.extend_from_slice(&[0; 6]);
        instruction_data.extend_from_slice(owner_keypair.pubkey().as_ref());

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(owner_auth_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ],
            data: {
                let mut data = vec![0];
                data.extend_from_slice(&instruction_data);
                data
            },
        };
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
        ).unwrap();
        let tx = VersionedTransaction::try_new(VersionedMessage::V0(msg), &[&context.payer]).unwrap();
        context.svm.send_transaction(tx).expect("CreateWallet failed");
    }

    // Fund vault
    {
        let ix = solana_sdk::system_instruction::transfer(&context.payer.pubkey(), &vault_pda, 1_000_000);
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
        ).unwrap();
        let tx = VersionedTransaction::try_new(VersionedMessage::V0(msg), &[&context.payer]).unwrap();
        context.svm.send_transaction(tx).expect("Fund vault failed");
    }

    // Create session
    let session_keypair = Keypair::new();
    let current_slot = context.svm.get_sysvar::<solana_sdk::clock::Clock>().slot;
    let expires_at = current_slot + 1000;

    let (session_pda, _) = Pubkey::find_program_address(
        &[b"session", wallet_pda.as_ref(), session_keypair.pubkey().as_ref()],
        &context.program_id,
    );

    {
        let mut session_args = Vec::new();
        session_args.extend_from_slice(session_keypair.pubkey().as_ref());
        session_args.extend_from_slice(&expires_at.to_le_bytes());

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(owner_auth_pda, false),
                AccountMeta::new(session_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new_readonly(owner_keypair.pubkey(), true),
            ],
            data: {
                let mut data = vec![5];
                data.extend_from_slice(&session_args);
                data
            },
        };
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
        ).unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(msg), &[&context.payer, &owner_keypair],
        ).unwrap();
        context.svm.send_transaction(tx).expect("CreateSession failed");
    }

    (wallet_pda, vault_pda, owner_auth_pda, owner_keypair, session_pda, session_keypair)
}

/// Build a RevokeSession instruction for Ed25519 auth
fn build_revoke_session_ix(
    program_id: &Pubkey,
    payer: &Pubkey,
    wallet_pda: &Pubkey,
    admin_auth_pda: &Pubkey,
    session_pda: &Pubkey,
    refund_dest: &Pubkey,
    signer_pubkey: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*wallet_pda, false),
            AccountMeta::new(*admin_auth_pda, false),
            AccountMeta::new(*session_pda, false),
            AccountMeta::new(*refund_dest, false),
            AccountMeta::new_readonly(*signer_pubkey, true),
        ],
        data: vec![9], // RevokeSession discriminator
    }
}

#[test]
fn test_revoke_session_by_owner() {
    let mut context = setup_test();
    let (wallet_pda, _vault, owner_auth_pda, owner_kp, session_pda, _session_kp) =
        setup_wallet_with_session(&mut context);

    // Verify session exists before revoke
    let session_account = context.svm.get_account(&session_pda);
    assert!(session_account.is_some(), "Session should exist before revoke");
    assert!(session_account.unwrap().lamports > 0);

    // Revoke session
    let refund_dest = context.payer.pubkey();
    let ix = build_revoke_session_ix(
        &context.program_id, &context.payer.pubkey(),
        &wallet_pda, &owner_auth_pda, &session_pda, &refund_dest,
        &owner_kp.pubkey(),
    );

    let msg = v0::Message::try_compile(
        &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
    ).unwrap();
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(msg), &[&context.payer, &owner_kp],
    ).unwrap();
    context.svm.send_transaction(tx).expect("RevokeSession failed");

    // Verify session is closed
    let session_account = context.svm.get_account(&session_pda);
    assert!(
        session_account.is_none() || session_account.unwrap().lamports == 0,
        "Session should be closed after revoke"
    );
    println!("✅ Owner revoked session successfully");
}

#[test]
fn test_revoke_session_by_admin() {
    let mut context = setup_test();
    let (wallet_pda, _vault, owner_auth_pda, owner_kp, session_pda, _session_kp) =
        setup_wallet_with_session(&mut context);

    // Add an admin authority
    let admin_kp = Keypair::new();
    let (admin_auth_pda, _) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), admin_kp.pubkey().as_ref()],
        &context.program_id,
    );

    {
        let mut add_data = Vec::new();
        add_data.push(0); // Ed25519
        add_data.push(1); // Admin role
        add_data.extend_from_slice(&[0; 6]); // padding
        add_data.extend_from_slice(admin_kp.pubkey().as_ref());

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(owner_auth_pda, false),
                AccountMeta::new(admin_auth_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new_readonly(owner_kp.pubkey(), true),
            ],
            data: {
                let mut data = vec![1]; // AddAuthority
                data.extend_from_slice(&add_data);
                data
            },
        };
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
        ).unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(msg), &[&context.payer, &owner_kp],
        ).unwrap();
        context.svm.send_transaction(tx).expect("AddAuthority (admin) failed");
    }

    // Admin revokes session
    let refund_dest = context.payer.pubkey();
    let ix = build_revoke_session_ix(
        &context.program_id, &context.payer.pubkey(),
        &wallet_pda, &admin_auth_pda, &session_pda, &refund_dest,
        &admin_kp.pubkey(),
    );
    let msg = v0::Message::try_compile(
        &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
    ).unwrap();
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(msg), &[&context.payer, &admin_kp],
    ).unwrap();
    context.svm.send_transaction(tx).expect("Admin RevokeSession failed");

    // Verify closed
    let session_account = context.svm.get_account(&session_pda);
    assert!(
        session_account.is_none() || session_account.unwrap().lamports == 0,
        "Session should be closed after admin revoke"
    );
    println!("✅ Admin revoked session successfully");
}

#[test]
fn test_revoke_session_spender_fails() {
    let mut context = setup_test();
    let (wallet_pda, _vault, owner_auth_pda, owner_kp, session_pda, _session_kp) =
        setup_wallet_with_session(&mut context);

    // Add a spender
    let spender_kp = Keypair::new();
    let (spender_auth_pda, _) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), spender_kp.pubkey().as_ref()],
        &context.program_id,
    );

    {
        let mut add_data = Vec::new();
        add_data.push(0); // Ed25519
        add_data.push(2); // Spender role
        add_data.extend_from_slice(&[0; 6]);
        add_data.extend_from_slice(spender_kp.pubkey().as_ref());

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(owner_auth_pda, false),
                AccountMeta::new(spender_auth_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new_readonly(owner_kp.pubkey(), true),
            ],
            data: {
                let mut data = vec![1];
                data.extend_from_slice(&add_data);
                data
            },
        };
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
        ).unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(msg), &[&context.payer, &owner_kp],
        ).unwrap();
        context.svm.send_transaction(tx).expect("AddAuthority (spender) failed");
    }

    // Spender tries to revoke — should fail
    let refund_dest = context.payer.pubkey();
    let ix = build_revoke_session_ix(
        &context.program_id, &context.payer.pubkey(),
        &wallet_pda, &spender_auth_pda, &session_pda, &refund_dest,
        &spender_kp.pubkey(),
    );
    let msg = v0::Message::try_compile(
        &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
    ).unwrap();
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(msg), &[&context.payer, &spender_kp],
    ).unwrap();
    let result = context.svm.send_transaction(tx);
    assert!(result.is_err(), "Spender should not be able to revoke session");
    println!("✅ Spender revoke correctly rejected");
}

#[test]
fn test_revoke_session_wrong_wallet_fails() {
    let mut context = setup_test();

    // Create two wallets — use wallet A's owner to revoke wallet B's session
    let (wallet_a, _, owner_auth_a, owner_kp_a, _, _) =
        setup_wallet_with_session(&mut context);
    let (_wallet_b, _, _, _, session_pda_b, _) =
        setup_wallet_with_session(&mut context);

    // Try to revoke wallet B's session using wallet A's authority
    let refund_dest = context.payer.pubkey();
    let ix = build_revoke_session_ix(
        &context.program_id, &context.payer.pubkey(),
        &wallet_a, &owner_auth_a, &session_pda_b, &refund_dest,
        &owner_kp_a.pubkey(),
    );
    let msg = v0::Message::try_compile(
        &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
    ).unwrap();
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(msg), &[&context.payer, &owner_kp_a],
    ).unwrap();
    let result = context.svm.send_transaction(tx);
    assert!(result.is_err(), "Cross-wallet session revoke should fail");
    println!("✅ Cross-wallet revoke correctly rejected");
}

#[test]
fn test_execute_after_revocation_fails() {
    let mut context = setup_test();
    let (wallet_pda, vault_pda, owner_auth_pda, owner_kp, session_pda, session_kp) =
        setup_wallet_with_session(&mut context);

    // Execute with session BEFORE revoke — should succeed
    {
        let transfer_amount = 1000u64;
        let mut transfer_data = Vec::new();
        transfer_data.extend_from_slice(&2u32.to_le_bytes());
        transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());

        let compact_ix = CompactInstruction {
            program_id_index: 6,
            accounts: vec![4, 5, 6],
            data: transfer_data,
        };
        let compact_bytes = compact::serialize_compact_instructions(&[compact_ix]);

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(session_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(context.payer.pubkey(), false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(session_kp.pubkey(), true),
            ],
            data: {
                let mut data = vec![4];
                data.extend_from_slice(&compact_bytes);
                data
            },
        };
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
        ).unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(msg), &[&context.payer, &session_kp],
        ).unwrap();
        context.svm.send_transaction(tx).expect("Session execute before revoke should succeed");
    }
    println!("✅ Session execute succeeded before revoke");

    // Revoke session
    {
        let refund_dest = context.payer.pubkey();
        let ix = build_revoke_session_ix(
            &context.program_id, &context.payer.pubkey(),
            &wallet_pda, &owner_auth_pda, &session_pda, &refund_dest,
            &owner_kp.pubkey(),
        );
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
        ).unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(msg), &[&context.payer, &owner_kp],
        ).unwrap();
        context.svm.send_transaction(tx).expect("RevokeSession failed");
    }

    // Try to execute with the same session AFTER revoke — should fail
    {
        let transfer_amount = 1000u64;
        let mut transfer_data = Vec::new();
        transfer_data.extend_from_slice(&2u32.to_le_bytes());
        transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());

        let compact_ix = CompactInstruction {
            program_id_index: 6,
            accounts: vec![4, 5, 6],
            data: transfer_data,
        };
        let compact_bytes = compact::serialize_compact_instructions(&[compact_ix]);

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(session_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(context.payer.pubkey(), false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(session_kp.pubkey(), true),
            ],
            data: {
                let mut data = vec![4];
                data.extend_from_slice(&compact_bytes);
                data
            },
        };
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(), &[ix], &[], context.svm.latest_blockhash(),
        ).unwrap();
        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(msg), &[&context.payer, &session_kp],
        ).unwrap();
        let result = context.svm.send_transaction(tx);
        assert!(result.is_err(), "Execute with revoked session should fail");
    }
    println!("✅ Execute after revocation correctly rejected");
}
