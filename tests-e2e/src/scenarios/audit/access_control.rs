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
    println!("\n🔐 Running Audit: Access Control & Validation Tests...");

    test_issue_1_create_wallet_checks(ctx)?;
    test_n2_fake_system_program(ctx)?;
    test_issue_6_parsing_failures(ctx)?;
    test_issue_15_invalid_new_owner(ctx)?;
    test_issue_7_wallet_discriminator(ctx)?;
    test_issue_3_cross_wallet_attacks(ctx)?;

    Ok(())
}

/// Issue #1: Verify CreateWallet fails if ownership is invalid (e.g. wrong derived address)
fn test_issue_1_create_wallet_checks(ctx: &mut TestContext) -> Result<()> {
    println!("\n[Issue #1] Testing CreateWallet ownership checks...");
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();
    let random_keypair = Keypair::new();
    let random_pda = random_keypair.pubkey();

    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    let mut data = vec![0];
    data.extend_from_slice(&user_seed);
    data.push(0);
    data.push(bump);
    data.extend_from_slice(&[0; 6]);
    data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());

    let ix_bad_wallet = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(random_pda.to_address(), false), // WRONG
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data: data.clone(),
    };
    let tx_bad_wallet = Transaction::new_signed_with_payer(
        &[ix_bad_wallet],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx_expect_error(tx_bad_wallet)?;
    println!("   ✓ Random Wallet PDA rejected (Issue #1 check verification)");

    Ok(())
}

/// N2: Test that fake system_program is rejected
fn test_n2_fake_system_program(ctx: &mut TestContext) -> Result<()> {
    println!("\n[N2] Testing fake system_program rejection...");

    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    let fake_system_program = Keypair::new();

    let mut data = vec![0];
    data.extend_from_slice(&user_seed);
    data.push(0);
    data.push(bump);
    data.extend_from_slice(&[0; 6]);
    data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());

    let create_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&fake_system_program).to_address(), false), // FAKE!
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data,
    };

    let message = Message::new(&[create_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut tx = Transaction::new_unsigned(message);
    tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(tx)?;
    println!("   ✓ Fake system_program rejected correctly");

    Ok(())
}

/// Issue #6: Test truncated instruction parsing (DoS/Panic prevention)
fn test_issue_6_parsing_failures(ctx: &mut TestContext) -> Result<()> {
    println!("\n[Issue #6] Testing Truncated Instruction Parsing...");

    let short_data = vec![0]; // Just discriminator

    let ix_short = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![AccountMeta::new(
            Signer::pubkey(&ctx.payer).to_address(),
            true,
        )],
        data: short_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix_short],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer],
        ctx.svm.latest_blockhash(),
    );

    ctx.execute_tx_expect_error(tx)?;
    println!("   ✓ Short/Truncated instruction data rejected safely");

    Ok(())
}

/// Issue #15: Verify TransferOwnership fails if new_owner is invalid (e.g. SystemProgram)
fn test_issue_15_invalid_new_owner(ctx: &mut TestContext) -> Result<()> {
    println!("\n[Issue #15] Testing Invalid New Owner...");

    // Setup generic wallet
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();
    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    let mut create_data = vec![0];
    create_data.extend_from_slice(&user_seed);
    create_data.push(0);
    create_data.push(bump);
    create_data.extend_from_slice(&[0; 6]);
    create_data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());
    let create_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data: create_data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx(tx)?;

    // Attempt invalid transfer
    let invalid_owner = solana_system_program::id();
    let (new_auth_pda, _) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), invalid_owner.as_ref()],
        &ctx.program_id,
    );

    let mut transfer_data = Vec::new();
    transfer_data.push(3); // Transfer
    transfer_data.push(0);
    transfer_data.extend_from_slice(invalid_owner.as_ref());

    let transfer_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false),
            AccountMeta::new(new_auth_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: transfer_data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer, &owner_keypair],
        ctx.svm.latest_blockhash(),
    );

    ctx.execute_tx_expect_error(tx)?;
    println!("   ✓ Invalid New Owner (System Program) rejected");

    Ok(())
}

/// Issue #7: Wallet Discriminator Check
fn test_issue_7_wallet_discriminator(ctx: &mut TestContext) -> Result<()> {
    println!("\n[Issue #7] Testing Wallet Discriminator Validation...");

    // Setup
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();
    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    // Create correct wallet first
    let mut data = vec![0];
    data.extend_from_slice(&user_seed);
    data.push(0);
    data.push(bump);
    data.extend_from_slice(&[0; 6]);
    data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());
    let create_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx(tx)?;

    // Use Authority PDA as FAKE Wallet PDA
    let fake_wallet_pda = owner_auth_pda;

    let bad_session_keypair = Keypair::new();
    let (bad_session_pda, _) = Pubkey::find_program_address(
        &[
            b"session",
            fake_wallet_pda.as_ref(),
            bad_session_keypair.pubkey().as_ref(),
        ],
        &ctx.program_id,
    );

    let clock: solana_clock::Clock = ctx.svm.get_sysvar();
    let current_slot = clock.slot;

    let mut bad_session_data = Vec::new();
    bad_session_data.push(5); // CreateSession
    bad_session_data.extend_from_slice(bad_session_keypair.pubkey().as_ref());
    bad_session_data.extend_from_slice(&(current_slot + 100).to_le_bytes());

    let bad_discriminator_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new_readonly(fake_wallet_pda.to_address(), false),
            AccountMeta::new_readonly(owner_auth_pda.to_address(), false),
            AccountMeta::new(bad_session_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: bad_session_data,
    };

    let message = Message::new(
        &[bad_discriminator_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut bad_disc_tx = Transaction::new_unsigned(message);
    bad_disc_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(bad_disc_tx)?;
    println!("   ✓ Invalid Wallet Discriminator Rejected.");

    Ok(())
}

/// Issue #3: Cross-Wallet Attacks
fn test_issue_3_cross_wallet_attacks(ctx: &mut TestContext) -> Result<()> {
    println!("\n[Issue #3] Testing Cross-Wallet Authority Checks...");

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

    // Create A
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
    let tx_a = Transaction::new_signed_with_payer(
        &[create_a_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx(tx_a)?;

    // Create B
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
    let tx_b = Transaction::new_signed_with_payer(
        &[create_b_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx(tx_b)?;

    // 1. Cross-Wallet Addition
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
    add_cross_data.push(1);
    add_cross_data.extend_from_slice(&[0; 6]);
    add_cross_data.extend_from_slice(Signer::pubkey(&attacker_keypair).as_ref());
    add_cross_data.extend_from_slice(Signer::pubkey(&attacker_keypair).as_ref());

    let cross_add_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_b.to_address(), false), // Target B
            AccountMeta::new(owner_a_auth.to_address(), false), // Auth A (Wrong)
            AccountMeta::new(attacker_auth_b.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_a).to_address(), true),
        ],
        data: add_cross_data,
    };
    let tx_cross_add = Transaction::new_signed_with_payer(
        &[cross_add_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer, &owner_a],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx_expect_error(tx_cross_add)?;
    println!("   ✓ Cross-Wallet Authority Addition Rejected.");

    // 2. Cross-Wallet Removal
    let remove_cross_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_b.to_address(), false), // Target B
            AccountMeta::new(owner_a_auth.to_address(), false), // Auth A (Wrong)
            AccountMeta::new(owner_b_auth.to_address(), false), // Target Owner B
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_a).to_address(), true),
        ],
        data: vec![2],
    };
    let tx_remove_cross = Transaction::new_signed_with_payer(
        &[remove_cross_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer, &owner_a],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx_expect_error(tx_remove_cross)?;
    println!("   ✓ Cross-Wallet Authority Removal Rejected.");

    Ok(())
}
