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
fn test_create_wallet_ed25519() {
    let mut context = setup_test();

    // Generate test data
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    // Derive PDAs
    let (wallet_pda, _wallet_bump) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);

    let (vault_pda, _vault_bump) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);

    let (auth_pda, auth_bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            owner_keypair.pubkey().as_ref(),
        ],
        &context.program_id,
    );

    // Build instruction data
    // Format: [user_seed(32)][authority_type(1)][role(1)][padding(6)][pubkey(32)]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&user_seed);
    instruction_data.push(0); // Ed25519
    instruction_data.push(auth_bump); // Owner role (bump)
    instruction_data.extend_from_slice(&[0; 6]); // padding
    instruction_data.extend_from_slice(owner_keypair.pubkey().as_ref());

    // Build CreateWallet instruction
    let create_wallet_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(auth_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
        ],
        data: {
            let mut data = vec![0]; // CreateWallet discriminator
            data.extend_from_slice(&instruction_data);
            data
        },
    };

    // Send transaction
    let message = v0::Message::try_compile(
        &context.payer.pubkey(),
        &[create_wallet_ix],
        &[],
        context.svm.latest_blockhash(),
    )
    .expect("Failed to compile message");

    let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&context.payer])
        .expect("Failed to create transaction");

    let result = context.svm.send_transaction(tx);

    if result.is_err() {
        let err = result.err().unwrap();
        eprintln!("Transaction failed:");
        eprintln!("{}", err.meta.pretty_logs());
        panic!("CreateWallet failed");
    }

    let metadata = result.unwrap();
    println!("✅ CreateWallet succeeded");
    println!("   CU consumed: {}", metadata.compute_units_consumed);

    // Verify wallet account exists
    let wallet_account = context.svm.get_account(&wallet_pda);
    assert!(wallet_account.is_some(), "Wallet account should exist");

    // Verify vault exists
    let vault_account = context.svm.get_account(&vault_pda);
    assert!(vault_account.is_some(), "Vault account should exist");

    // Verify authority exists
    let auth_account = context.svm.get_account(&auth_pda);
    assert!(auth_account.is_some(), "Authority account should exist");

    println!("✅ All accounts created successfully");
}

#[test]
fn test_compact_instructions_basic() {
    // Simple test to verify CompactInstructions work
    use lazorkit_program::compact::{
        parse_compact_instructions, serialize_compact_instructions, CompactInstruction,
    };

    let instructions = vec![CompactInstruction {
        program_id_index: 0,
        accounts: vec![1, 2],
        data: vec![1, 2, 3],
    }];

    let bytes = serialize_compact_instructions(&instructions);
    let parsed = parse_compact_instructions(&bytes).unwrap();

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].program_id_index, 0);
    assert_eq!(parsed[0].accounts, vec![1, 2]);
    assert_eq!(parsed[0].data, vec![1, 2, 3]);

    println!("✅ CompactInstructions serialization works");
}

#[test]
fn test_authority_lifecycle() {
    let mut context = setup_test();

    // 1. Create Wallet (Owner)
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
            owner_keypair.pubkey().as_ref(), // seed for Ed25519 is pubkey
        ],
        &context.program_id,
    );

    // Create Wallet Instruction
    {
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&user_seed);
        instruction_data.push(0); // Ed25519
        instruction_data.push(owner_bump); // Owner role (bump)
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
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
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
        .expect("Failed to compile create message");

        let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&context.payer])
            .expect("Failed to create create transaction");

        context
            .svm
            .send_transaction(tx)
            .expect("CreateWallet failed");
    }
    println!("✅ Wallet created with Owner");

    // 2. Add New Authority (Admin)
    let admin_keypair = Keypair::new();
    let (admin_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            admin_keypair.pubkey().as_ref(),
        ],
        &context.program_id,
    );

    {
        // AddAuthorityArgs: type(1) + role(1) + padding(6)
        let mut add_auth_args = Vec::new();
        add_auth_args.push(0); // Ed25519
        add_auth_args.push(1); // Admin role
        add_auth_args.extend_from_slice(&[0; 6]);

        // Data payload for Ed25519 is just the pubkey
        let mut instruction_payload = Vec::new();
        instruction_payload.extend_from_slice(&add_auth_args);
        instruction_payload.extend_from_slice(admin_keypair.pubkey().as_ref());

        // Auth payload (signature from Owner to authorize adding)
        // Since we are not using the aggregated signature verification yet (assuming it checks transaction signatures if provided?)
        // Wait, manage_authority checks signatures internally using ed25519_verify or similar?
        // Actually, LazorKit uses `check_signature` which verifies the transaction signature for Ed25519.
        // But for `AddAuthority`, we need to pass the "authority payload" at the end of instruction data?
        // Let's check manage_authority.rs again.
        // For Ed25519, the `authority_payload` is usually empty if we just rely on the account being a signer in the transaction.
        // BUT, the instruction data splitting in manage_authority looks like: `(args, rest) = AddAuthorityArgs::from_bytes`.
        // Then `rest` is split into `id_seed` (pubkey) and `authority_payload`.
        // So we need to append the authority payload.
        // For Ed25519 authorities, usually the payload is empty if it's just a standard signer check.
        // However, the `authenticate` function in `auth/ed25519.rs` might expect something.
        // Let's assume for now Ed25519 just needs to be a signer and payload is empty.

        let add_authority_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(owner_auth_pda, false), // PDA must be writable for auth logic
                AccountMeta::new(admin_auth_pda, false), // New authority PDA being created
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new_readonly(owner_keypair.pubkey(), true), // Actual signer
            ],
            data: {
                let mut data = vec![1]; // AddAuthority discriminator
                data.extend_from_slice(&instruction_payload);
                // No extra auth payload for Ed25519 simple signer?
                data
            },
        };

        let message = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[add_authority_ix],
            &[],
            context.svm.latest_blockhash(),
        )
        .expect("Failed to compile add_auth message");

        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            &[&context.payer, &owner_keypair], // Owner must sign
        )
        .expect("Failed to create add_auth transaction");

        let res = context.svm.send_transaction(tx);
        if let Err(e) = res {
            eprintln!("AddAuthority failed: {}", e.meta.pretty_logs());
            panic!("AddAuthority failed");
        }
    }
    println!("✅ Admin authority added");

    // Verify Admin PDA exists
    let admin_acc = context.svm.get_account(&admin_auth_pda);
    assert!(admin_acc.is_some(), "Admin authority PDA should exist");

    // 3. Remove Authority (Admin removes itself? Or Owner removes Admin?)
    // Let's have Owner remove Admin.
    {
        // For RemoveAuthority, instruction data is just "authority_payload" (for authentication).
        // Since Owner is Ed25519, payload matches what `authenticate` expects.
        // For Ed25519, `authenticate` expects empty payload if it relies on transaction signatures.

        let remove_authority_ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(owner_auth_pda, false), // PDA must be writable
                AccountMeta::new(admin_auth_pda, false), // Target to remove
                AccountMeta::new(context.payer.pubkey(), false), // Refund destination
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(owner_keypair.pubkey(), true), // Actual signer
            ],
            data: vec![2], // RemoveAuthority discriminator (and empty payload)
        };

        let message = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[remove_authority_ix],
            &[],
            context.svm.latest_blockhash(),
        )
        .expect("Failed to compile remove_auth message");

        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(message),
            &[&context.payer, &owner_keypair],
        )
        .expect("Failed to create remove_auth transaction");

        let res = context.svm.send_transaction(tx);
        if let Err(e) = res {
            eprintln!("RemoveAuthority failed: {}", e.meta.pretty_logs());
            panic!("RemoveAuthority failed");
        }
    }
    println!("✅ Admin authority removed");

    // Verify Admin PDA is gone (or data allows for re-initialization? Standard behavior is closing account)
    let admin_acc = context.svm.get_account(&admin_auth_pda);
    if let Some(acc) = &admin_acc {
        println!(
            "Admin Acc: Lamports={}, DataLen={}, Owner={}",
            acc.lamports,
            acc.data.len(),
            acc.owner
        );
        assert_eq!(
            acc.lamports, 0,
            "Admin authority PDA should have 0 lamports"
        );
    } else {
        // None is also acceptable (means fully purged)
    }
}

#[test]
fn test_execute_with_compact_instructions() {
    let mut context = setup_test();

    // 1. Setup Wallet & Authority
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

    // Create Wallet logic (simplified re-use)
    {
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&user_seed);
        instruction_data.push(0); // Ed25519
        instruction_data.push(owner_bump); // Owner role (bump)
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
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ],
            data: {
                let mut data = vec![0];
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
        .expect("Failed compile create");
        let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&context.payer])
            .unwrap();
        context.svm.send_transaction(tx).expect("Create failed");
    }

    // Fund Vault so it can transfer
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

    // 2. Prepare Compact Instruction (Transfer 5000 lamports from Vault to Payer)
    use lazorkit_program::compact::{self, CompactInstruction};

    // Inner accounts indices (relative to the slice passed after fixed accounts)
    // 0: Vault (Signer)
    // 1: Payer (Destination)
    // 2: SystemProgram

    let transfer_amount = 5000u64;
    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes()); // Transfer instruction discriminator (2)
    transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());

    let compact_ix = CompactInstruction {
        program_id_index: 6,
        accounts: vec![4, 5, 6], // Vault, Payer, SystemProgram
        data: transfer_data,
    };

    let compact_bytes = compact::serialize_compact_instructions(&[compact_ix]);

    // 3. Execute Instruction
    let execute_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true), // Payer
            AccountMeta::new(wallet_pda, false),            // Wallet
            AccountMeta::new(owner_auth_pda, false),        // Authority (PDA) must be writable
            AccountMeta::new(vault_pda, false),             // Vault (Context)
            // Inner accounts start here:
            AccountMeta::new(vault_pda, false), // Index 0: Vault (will satisfy Signer via seeds)
            AccountMeta::new(context.payer.pubkey(), false), // Index 1: Payer (Dest)
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // Index 2: SystemProgram
            // Authentication: Owner Keypair
            AccountMeta::new_readonly(owner_keypair.pubkey(), true), // Owner signs transaction
        ],
        data: {
            let mut data = vec![4]; // Execute discriminator
            data.extend_from_slice(&compact_bytes);
            // Ed25519 needs no extra payload, signature is validated against owner_keypair account
            data
        },
    };

    let message = v0::Message::try_compile(
        &context.payer.pubkey(),
        &[execute_ix],
        &[],
        context.svm.latest_blockhash(),
    )
    .expect("Failed compile execute");

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[&context.payer, &owner_keypair], // Owner signs
    )
    .expect("Failed create execute tx");

    let res = context.svm.send_transaction(tx);
    if let Err(e) = res {
        eprintln!("Execute failed: {}", e.meta.pretty_logs());
        panic!("Execute failed");
    }
    println!("✅ Execute transaction succeeded");
}
