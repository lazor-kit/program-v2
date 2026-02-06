use crate::common::{TestContext, ToAddress};
use anyhow::Result;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_program;
use solana_sysvar;
use solana_transaction::Transaction;

pub fn run(ctx: &mut TestContext) -> Result<()> {
    println!("\n🔒 Running Cross-Wallet Attack Scenarios...");

    // Setup: Create TWO wallets
    let user_seed_a = rand::random::<[u8; 32]>();
    let owner_a = Keypair::new();
    let (wallet_a, _) = Pubkey::find_program_address(&[b"wallet", &user_seed_a], &ctx.program_id);
    let (vault_a, _) =
        Pubkey::find_program_address(&[b"vault", wallet_a.as_ref()], &ctx.program_id);
    let (owner_a_auth, auth_bump_a) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_a.as_ref(),
            Signer::pubkey(&owner_a).as_ref(),
        ],
        &ctx.program_id,
    );

    let user_seed_b = rand::random::<[u8; 32]>();
    let owner_b = Keypair::new();
    let (wallet_b, _) = Pubkey::find_program_address(&[b"wallet", &user_seed_b], &ctx.program_id);
    let (vault_b, _) =
        Pubkey::find_program_address(&[b"vault", wallet_b.as_ref()], &ctx.program_id);
    let (owner_b_auth, auth_bump_b) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_b.as_ref(),
            Signer::pubkey(&owner_b).as_ref(),
        ],
        &ctx.program_id,
    );

    // Create Wallet A
    println!("\n[Setup] Creating Wallet A...");
    let mut data_a = vec![0];
    data_a.extend_from_slice(&user_seed_a);
    data_a.push(0);
    data_a.push(auth_bump_a);
    data_a.extend_from_slice(&[0; 6]);
    data_a.extend_from_slice(Signer::pubkey(&owner_a).as_ref());

    let create_a_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_a.to_address(), false),
            AccountMeta::new(vault_a.to_address(), false),
            AccountMeta::new(owner_a_auth.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data: data_a,
    };

    let message_a = Message::new(
        &[create_a_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut create_a_tx = Transaction::new_unsigned(message_a);
    create_a_tx.sign(&[&ctx.payer, &owner_a], ctx.svm.latest_blockhash());

    ctx.execute_tx(create_a_tx)?;

    // Create Wallet B
    println!("[Setup] Creating Wallet B...");
    let mut data_b = vec![0];
    data_b.extend_from_slice(&user_seed_b);
    data_b.push(0);
    data_b.push(auth_bump_b);
    data_b.extend_from_slice(&[0; 6]);
    data_b.extend_from_slice(Signer::pubkey(&owner_b).as_ref());

    let create_b_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_b.to_address(), false),
            AccountMeta::new(vault_b.to_address(), false),
            AccountMeta::new(owner_b_auth.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data: data_b,
    };

    let message_b = Message::new(
        &[create_b_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut create_b_tx = Transaction::new_unsigned(message_b);
    create_b_tx.sign(&[&ctx.payer, &owner_b], ctx.svm.latest_blockhash());

    ctx.execute_tx(create_b_tx)?;

    // Scenario 1: Owner A tries to Add Authority to Wallet B
    println!("\n[1/3] Testing Cross-Wallet Authority Addition...");
    let attacker_keypair = Keypair::new();
    let (attacker_auth_b, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_b.as_ref(),
            Signer::pubkey(&attacker_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    let mut add_cross_data = vec![1];
    add_cross_data.push(0);
    add_cross_data.push(1); // Admin
    add_cross_data.extend_from_slice(&[0; 6]);
    add_cross_data.extend_from_slice(Signer::pubkey(&attacker_keypair).as_ref());
    add_cross_data.extend_from_slice(Signer::pubkey(&attacker_keypair).as_ref());

    let cross_add_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_b.to_address(), false), // Target: Wallet B
            AccountMeta::new(owner_a_auth.to_address(), false), // Auth: Owner A (WRONG WALLET)
            AccountMeta::new(attacker_auth_b.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_a).to_address(), true), // Owner A signing
        ],
        data: vec![1, 1],
    };

    let message_cross = Message::new(
        &[cross_add_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut cross_add_tx = Transaction::new_unsigned(message_cross);
    cross_add_tx.sign(&[&ctx.payer, &owner_a], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(cross_add_tx)?;
    println!("✅ Cross-Wallet Authority Addition Rejected.");

    // Scenario 2: Owner A tries to Remove Owner from Wallet B
    println!("\n[2/3] Testing Cross-Wallet Authority Removal...");
    let remove_cross_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_b.to_address(), false), // Target: Wallet B
            AccountMeta::new(owner_a_auth.to_address(), false), // Auth: Owner A (WRONG)
            AccountMeta::new(owner_b_auth.to_address(), false), // Target: Owner B
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_a).to_address(), true),
        ],
        data: vec![2],
    };

    let message_remove = Message::new(
        &[remove_cross_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut remove_cross_tx = Transaction::new_unsigned(message_remove);
    remove_cross_tx.sign(&[&ctx.payer, &owner_a], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(remove_cross_tx)?;
    println!("✅ Cross-Wallet Authority Removal Rejected.");

    // Scenario 3: Owner A tries to Execute on Wallet B's Vault
    println!("\n[3/3] Testing Cross-Wallet Execution...");
    let mut exec_cross_data = vec![4];
    exec_cross_data.push(0); // Empty compact

    let exec_cross_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_b.to_address(), false), // Target: Wallet B
            AccountMeta::new(owner_a_auth.to_address(), false), // Auth: Owner A (WRONG)
            AccountMeta::new(vault_b.to_address(), false),  // Vault B
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_a).to_address(), true),
        ],
        data: exec_cross_data,
    };

    let message_exec = Message::new(
        &[exec_cross_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut exec_cross_tx = Transaction::new_unsigned(message_exec);
    exec_cross_tx.sign(&[&ctx.payer, &owner_a], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(exec_cross_tx)?;
    println!("✅ Cross-Wallet Execution Rejected.");

    println!("\n✅ All Cross-Wallet Attack Scenarios Passed!");
    Ok(())
}
