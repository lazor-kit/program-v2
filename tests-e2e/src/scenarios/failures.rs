use crate::common::{TestContext, ToAddress};
use anyhow::Result;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_program;
use solana_sysvar;
// use solana_transaction::Transaction; // Transaction usage needs refactor
use solana_message::Message;
use solana_transaction::Transaction;

pub fn run(ctx: &mut TestContext) -> Result<()> {
    println!("\n🛡️  Running Failure Scenarios...");

    // Setup: Create a separate wallet for these tests
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();
    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);

    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, auth_bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(), // Explicit Signer call
        ],
        &ctx.program_id,
    );

    // Create Wallet
    let mut data = Vec::new();
    data.push(0);
    data.extend_from_slice(&user_seed);
    data.push(0);
    data.push(auth_bump);
    data.extend_from_slice(&[0; 6]);
    data.extend_from_slice(owner_keypair.pubkey().as_ref());

    let create_wallet_ix = Instruction {
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

    let clock: solana_clock::Clock = ctx.svm.get_sysvar();
    let _now = clock.unix_timestamp as u64; // Corrected variable name and cleaned up

    let latest_blockhash = ctx.svm.latest_blockhash();
    let message = Message::new(
        &[create_wallet_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut create_wallet_tx = Transaction::new_unsigned(message);
    create_wallet_tx.sign(&[&ctx.payer], latest_blockhash);

    ctx.execute_tx(create_wallet_tx)?;

    // Scenario 1: Replay Vulnerability Check (Read-Only Authority)
    // Attempt to pass Authority as Read-Only to bypass `writable` check.
    // This was the vulnerability we fixed.
    println!("\n[1/3] Testing Replay Vulnerability (Read-Only Authority)...");

    // Construct Execute instruction
    let mut exec_data = vec![4]; // Execute
                                 // Empty compact instructions (just testing auth check)
    exec_data.push(0); // 0 instructions

    // Create instruction with Read-Only Authority
    let replay_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            // FAIL TARGET: Read-Only Authority
            AccountMeta::new_readonly(owner_auth_pda.to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            // Signer
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: exec_data,
    };

    let message = Message::new(&[replay_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut replay_tx = Transaction::new_unsigned(message);
    replay_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(replay_tx)?;
    println!("✅ Read-Only Authority Rejected (Replay Protection Active).");

    // Scenario 2: Invalid Signer
    println!("\n[2/3] Testing Invalid Signer...");
    let fake_signer = Keypair::new();
    let (fake_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&fake_signer).as_ref(),
        ],
        &ctx.program_id,
    );
    let invalid_signer_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false), // auth
            AccountMeta::new(fake_auth_pda.to_address(), false),  // target
            AccountMeta::new(Signer::pubkey(&fake_signer).to_address(), true), // WRONG SIGNER
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
        ],
        data: vec![1, 2], // AddAuthority(Spender)
    };

    let message = Message::new(
        &[invalid_signer_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut invalid_signer_tx = Transaction::new_unsigned(message);
    invalid_signer_tx.sign(&[&ctx.payer, &fake_signer], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(invalid_signer_tx)?;
    println!("✅ Invalid Signer Rejected.");

    // Scenario 3: Spender Privilege Escalation (Add Authority)
    println!("\n[3/3] Testing Spender Privilege Escalation...");
    // First Add a Spender
    let spender_keypair = Keypair::new();
    let (spender_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&spender_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    // Add Spender (by Owner)
    let mut add_spender_data = vec![1]; // AddAuthority
    add_spender_data.push(0); // Ed25519
    add_spender_data.push(2); // Spender Role
    add_spender_data.extend_from_slice(&[0; 6]);
    add_spender_data.extend_from_slice(spender_keypair.pubkey().as_ref()); // Seed
    add_spender_data.extend_from_slice(spender_keypair.pubkey().as_ref()); // Pubkey

    let add_spender_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new_readonly(owner_auth_pda.to_address(), false), // auth
            AccountMeta::new(spender_auth_pda.to_address(), false),        // target
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true), // signer
        ],
        data: add_spender_data,
    };

    let message = Message::new(
        &[add_spender_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut add_spender_tx = Transaction::new_unsigned(message);
    add_spender_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(add_spender_tx)?;
    println!("   -> Spender Added.");

    // Now Spender tries to Add another Authority (Admin)
    let bad_admin_keypair = Keypair::new();
    let (bad_admin_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&bad_admin_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    let mut malicious_add = vec![1];
    malicious_add.push(0);
    malicious_add.push(1); // Try to add Admin
    malicious_add.extend_from_slice(&[0; 6]);
    malicious_add.extend_from_slice(bad_admin_keypair.pubkey().as_ref());
    malicious_add.extend_from_slice(bad_admin_keypair.pubkey().as_ref());

    let malicious_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new_readonly(spender_auth_pda.to_address(), false), // Spender auth
            AccountMeta::new(bad_admin_pda.to_address(), false),             // Target (Bad admin)
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&spender_keypair).to_address(), true), // Signer
        ],
        data: malicious_add,
    };

    let message = Message::new(
        &[malicious_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut malicious_tx = Transaction::new_unsigned(message);
    malicious_tx.sign(&[&ctx.payer, &spender_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(malicious_tx)?;
    println!("✅ Spender Escalation Rejected.");

    // Scenario 4: Session Expiry
    println!("\n[4/5] Testing Session Expiry...");
    // Create Expired Session
    let session_keypair = Keypair::new();
    let (session_pda, _) = Pubkey::find_program_address(
        &[
            b"session",
            wallet_pda.as_ref(),
            session_keypair.pubkey().as_ref(),
        ],
        &ctx.program_id,
    );
    // Use slot-based expiry
    let clock: solana_clock::Clock = ctx.svm.get_sysvar();
    let current_slot = clock.slot;
    let expires_at = current_slot + 50; // Expires in 50 slots

    let mut session_data = Vec::new();
    session_data.push(5); // CreateSession
    session_data.extend_from_slice(session_keypair.pubkey().as_ref());
    session_data.extend_from_slice(&expires_at.to_le_bytes());

    let create_session_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new_readonly(wallet_pda.to_address(), false),
            AccountMeta::new_readonly(owner_auth_pda.to_address(), false),
            AccountMeta::new(session_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: session_data,
    };

    let message = Message::new(
        &[create_session_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut create_session_tx = Transaction::new_unsigned(message);
    create_session_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(create_session_tx)?;

    // Warp to future slot to expire session
    ctx.warp_to_slot(current_slot + 100);

    // Try to Execute with Expired Session
    let mut exec_payload = vec![4]; // Execute
    exec_payload.push(0); // Empty compact instructions
    let exec_expired_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true), // Payer
            AccountMeta::new(wallet_pda.to_address(), false),                // Wallet
            AccountMeta::new(session_pda.to_address(), false), // Authority (Session PDA)
            AccountMeta::new(vault_pda.to_address(), false),   // Vault
            AccountMeta::new_readonly(Signer::pubkey(&session_keypair).to_address(), true), // Session Signer
        ],
        data: exec_payload,
    };

    let message = Message::new(
        &[exec_expired_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut exec_expired_tx = Transaction::new_unsigned(message);
    exec_expired_tx.sign(&[&ctx.payer, &session_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(exec_expired_tx)?;
    println!("✅ Expired Session Rejected.");

    // Scenario 5: Admin Permission Constraints
    println!("\n[5/5] Testing Admin vs Owner Permission...");
    // Create an Admin
    let admin_keypair = Keypair::new();
    let (admin_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&admin_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    // Owner creates Admin
    let mut add_admin_data = vec![1]; // AddAuthority
    add_admin_data.push(0); // Ed25519
    add_admin_data.push(1); // Admin Role
    add_admin_data.extend_from_slice(&[0; 6]);
    add_admin_data.extend_from_slice(admin_keypair.pubkey().as_ref());
    add_admin_data.extend_from_slice(admin_keypair.pubkey().as_ref());

    let add_admin_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new_readonly(owner_auth_pda.to_address(), false), // auth
            AccountMeta::new(admin_auth_pda.to_address(), false),          // target
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true), // signer
        ],
        data: add_admin_data,
    };

    let message = Message::new(
        &[add_admin_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut add_admin_tx = Transaction::new_unsigned(message);
    add_admin_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(add_admin_tx)?;
    println!("   -> Admin Added.");

    // Admin tries to Remove Owner
    let remove_owner_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(admin_auth_pda.to_address(), false), // Admin authorizes
            AccountMeta::new(owner_auth_pda.to_address(), false), // Target (Owner)
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), false), // Refund dest
            AccountMeta::new_readonly(Signer::pubkey(&admin_keypair).to_address(), true), // Signer
        ],
        data: vec![2], // RemoveAuthority
    };

    let message = Message::new(
        &[remove_owner_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut remove_owner_tx = Transaction::new_unsigned(message);
    remove_owner_tx.sign(&[&ctx.payer, &admin_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx_expect_error(remove_owner_tx)?;
    println!("✅ Admin Removing Owner Rejected.");

    Ok(())
}
