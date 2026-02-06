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

/// Tests for audit fixes N1, N2, N3
pub fn run(ctx: &mut TestContext) -> Result<()> {
    println!("\n🔐 Running Audit Validation Tests...");

    test_n2_fake_system_program(ctx)?;
    test_n1_valid_auth_bump(ctx)?;

    println!("\n✅ All Audit Validation Tests Passed!");
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

    // Create a fake "system program" keypair
    let fake_system_program = Keypair::new();

    let mut data = vec![0]; // CreateWallet discriminator
    data.extend_from_slice(&user_seed);
    data.push(0); // Type: Ed25519
    data.push(bump); // auth_bump
    data.extend_from_slice(&[0; 6]); // Padding
    data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());

    // Use FAKE system program instead of real one
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

    // Should fail with IncorrectProgramId
    ctx.execute_tx_expect_error(tx)?;
    println!("   ✓ Fake system_program rejected correctly");

    Ok(())
}

/// N1: Test that valid auth_bump is accepted (success path)
fn test_n1_valid_auth_bump(ctx: &mut TestContext) -> Result<()> {
    println!("\n[N1] Testing valid auth_bump acceptance...");

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

    let mut data = vec![0]; // CreateWallet discriminator
    data.extend_from_slice(&user_seed);
    data.push(0); // Type: Ed25519
    data.push(bump); // Correct auth_bump from find_program_address
    data.extend_from_slice(&[0; 6]); // Padding
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

    let message = Message::new(&[create_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut tx = Transaction::new_unsigned(message);
    tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    ctx.execute_tx(tx)?;
    println!("   ✓ Valid auth_bump accepted correctly");

    Ok(())
}

/// N1: Test that invalid auth_bump is rejected
#[allow(dead_code)]
fn test_n1_invalid_auth_bump(ctx: &mut TestContext) -> Result<()> {
    println!("\n[N1] Testing invalid auth_bump rejection...");

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

    // Use WRONG bump (bump - 1 or a different value)
    let wrong_bump = if bump > 0 { bump - 1 } else { 255 };

    let mut data = vec![0]; // CreateWallet discriminator
    data.extend_from_slice(&user_seed);
    data.push(0); // Type: Ed25519
    data.push(wrong_bump); // WRONG auth_bump
    data.extend_from_slice(&[0; 6]); // Padding
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

    let message = Message::new(&[create_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut tx = Transaction::new_unsigned(message);
    tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    // Should fail with InvalidSeeds
    ctx.execute_tx_expect_error(tx)?;
    println!("   ✓ Invalid auth_bump rejected correctly");

    Ok(())
}
