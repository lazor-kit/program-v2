use crate::common::{TestContext, ToAddress};
use anyhow::Result;
use solana_clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_program;
use solana_sysvar;
use solana_transaction::Transaction;

pub fn run(ctx: &mut TestContext) -> Result<()> {
    println!("\n🛡️ Running Audit: DoS & Rent Tests...");

    test_dos_attack(ctx)?;
    test_issue_5_rent_dependency(ctx)?;

    Ok(())
}

fn test_dos_attack(ctx: &mut TestContext) -> Result<()> {
    println!("\n[Issue #4] DoS Attack Mitigation Scenario...");

    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    // 1. Calculate PDA addresses
    let (wallet_pda, _wallet_bump) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (auth_pda, auth_bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    println!("   Target Wallet PDA: {}", wallet_pda);

    // 2. DoS Attack: Pre-fund the wallet PDA
    println!("   🔫 Attacker pre-funds Wallet PDA with 1 lamport...");

    let amount = 1u64;
    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes());
    transfer_data.extend_from_slice(&amount.to_le_bytes());

    let fund_ix = Instruction {
        program_id: solana_system_program::id().to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
        ],
        data: transfer_data,
    };

    let msg = Message::new(&[fund_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut fund_tx = Transaction::new_unsigned(msg);
    fund_tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    ctx.execute_tx(fund_tx)?;
    println!("   ✓ Wallet PDA pre-funded.");

    // 3. Attempt to Create Wallet (Should succeed now)
    println!("   🛡️ Victim attempts to create wallet...");

    let mut data = vec![0]; // CreateWallet discriminator
    data.extend_from_slice(&user_seed);
    data.push(0); // Ed25519
    data.push(auth_bump);
    data.extend_from_slice(&[0; 6]); // Padding
    data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());

    let create_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(auth_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data,
    };

    let msg = Message::new(&[create_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut create_tx = Transaction::new_unsigned(msg);
    create_tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    ctx.execute_tx(create_tx)?;
    println!("   ✓ Wallet creation SUCCESS (DoS mitigated).");

    // 4. Attempt to Create Session (Should succeed now)
    println!("\n   🛡️ Testing Create Session DoS Mitigation...");

    let session_keypair = Keypair::new();
    let (session_pda, _) = Pubkey::find_program_address(
        &[
            b"session",
            wallet_pda.as_ref(),
            Signer::pubkey(&session_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    // Pre-fund Session PDA
    println!("   🔫 Attacker pre-funds Session PDA with 1 lamport...");
    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes());
    transfer_data.extend_from_slice(&1u64.to_le_bytes());

    let fund_ix = Instruction {
        program_id: solana_system_program::id().to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(session_pda.to_address(), false),
        ],
        data: transfer_data,
    };

    let msg = Message::new(&[fund_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut fund_tx = Transaction::new_unsigned(msg);
    fund_tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    ctx.execute_tx(fund_tx)?;
    println!("   ✓ Session PDA pre-funded.");

    // Attempt Create Session
    println!("   🛡️ Attempting to create session...");
    let clock: solana_clock::Clock = ctx.svm.get_sysvar();
    let expires_at = clock.slot + 1000;

    let mut session_data = Vec::new();
    session_data.push(5); // CreateSession
    session_data.extend_from_slice(Signer::pubkey(&session_keypair).as_ref());
    session_data.extend_from_slice(&expires_at.to_le_bytes());

    let session_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new_readonly(wallet_pda.to_address(), false),
            AccountMeta::new_readonly(auth_pda.to_address(), false),
            AccountMeta::new(session_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: session_data,
    };

    let message_session = Message::new(
        &[session_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut session_tx = Transaction::new_unsigned(message_session);
    session_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(session_tx)?;
    println!("   ✓ Session creation SUCCESS (DoS mitigated).");

    Ok(())
}

/// Issue #5: Verify TransferOwnership fails if Rent sysvar is missing (proving it's required)
fn test_issue_5_rent_dependency(ctx: &mut TestContext) -> Result<()> {
    println!("\n[Issue #5] Testing TransferOwnership Rent dependency...");

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

    let msg = Message::new(&[create_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut tx = Transaction::new_unsigned(msg);
    tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());
    ctx.execute_tx(tx)?;

    // Try Transfer Ownership WITHOUT Rent sysvar
    println!("   -> Attempting transfer without Rent sysvar (expect failure)...");
    let new_owner = Keypair::new();
    let (new_owner_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&new_owner).as_ref(),
        ],
        &ctx.program_id,
    );

    let mut transfer_data = Vec::new();
    transfer_data.push(3); // TransferOwnership
    transfer_data.push(0); // Ed25519
    transfer_data.extend_from_slice(Signer::pubkey(&new_owner).as_ref());

    let transfer_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false),
            AccountMeta::new(new_owner_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            // MISSING RENT SYSVAR HERE
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: transfer_data,
    };

    let msg_transfer = Message::new(
        &[transfer_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut tx_transfer = Transaction::new_unsigned(msg_transfer);
    tx_transfer.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(tx_transfer)?;
    println!("   ✓ Transfer failed without Rent sysvar as expected (proving usage)");

    Ok(())
}
