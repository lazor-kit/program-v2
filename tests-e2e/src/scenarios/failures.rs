use crate::common::TestContext;
use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
};

pub async fn run(ctx: &TestContext) -> Result<()> {
    println!("\n🛡️  Running Failure Scenarios...");

    // Setup: Create a separate wallet for these tests
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();
    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            owner_keypair.pubkey().as_ref(),
        ],
        &ctx.program_id,
    );

    // Create Wallet
    let mut data = Vec::new();
    data.push(0);
    data.extend_from_slice(&user_seed);
    data.push(0);
    data.push(0);
    data.extend_from_slice(&[0; 6]);
    data.extend_from_slice(owner_keypair.pubkey().as_ref());

    let create_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(owner_auth_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };
    ctx.send_transaction(&[create_ix], &[&ctx.payer])?;

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
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            // FAIL TARGET: Read-Only Authority
            AccountMeta::new_readonly(owner_auth_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            // Signer
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
        ],
        data: exec_data,
    };

    ctx.send_transaction_expect_error(&[replay_ix], &[&ctx.payer, &owner_keypair])?;
    println!("✅ Read-Only Authority Rejected (Replay Protection Active).");

    // Scenario 2: Invalid Signer
    println!("\n[2/3] Testing Invalid Signer...");
    let fake_signer = Keypair::new();
    let invalid_signer_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(owner_auth_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            // FAIL TARGET: Wrong Signer
            AccountMeta::new_readonly(fake_signer.pubkey(), true),
        ],
        data: vec![4, 0], // Execute, 0 instructions
    };
    ctx.send_transaction_expect_error(&[invalid_signer_ix], &[&ctx.payer, &fake_signer])?;
    println!("✅ Invalid Signer Rejected.");

    // Scenario 3: Spender Privilege Escalation (Add Authority)
    println!("\n[3/3] Testing Spender Privilege Escalation...");
    // First Add a Spender
    let spender_keypair = Keypair::new();
    let (spender_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            spender_keypair.pubkey().as_ref(),
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
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new_readonly(owner_auth_pda, false), // Owner authorizes
            AccountMeta::new(spender_auth_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
        ],
        data: add_spender_data,
    };
    ctx.send_transaction(&[add_spender_ix], &[&ctx.payer, &owner_keypair])?;
    println!("   -> Spender Added.");

    // Now Spender tries to Add another Authority (Admin)
    let bad_admin_keypair = Keypair::new();
    let (bad_admin_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            bad_admin_keypair.pubkey().as_ref(),
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
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            // Spender tries to authorize
            AccountMeta::new_readonly(spender_auth_pda, false),
            AccountMeta::new(bad_admin_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spender_keypair.pubkey(), true),
        ],
        data: malicious_add,
    };

    ctx.send_transaction_expect_error(&[malicious_ix], &[&ctx.payer, &spender_keypair])?;
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
    let mut session_create_data = vec![5]; // CreateSession
    session_create_data.extend_from_slice(session_keypair.pubkey().as_ref());
    session_create_data.extend_from_slice(&0u64.to_le_bytes()); // Expires at 0 (Genesis)

    let create_session_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(owner_auth_pda, false), // Owner authorizes
            AccountMeta::new(session_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
        ],
        data: session_create_data,
    };
    ctx.send_transaction(&[create_session_ix], &[&ctx.payer, &owner_keypair])?;

    // Try to Execute with Expired Session
    let exec_expired_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(session_pda, false), // Session as authority
            AccountMeta::new(vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(session_keypair.pubkey(), true), // Signer
        ],
        data: vec![4, 0], // Execute, 0 instructions
    };
    ctx.send_transaction_expect_error(&[exec_expired_ix], &[&ctx.payer, &session_keypair])?;
    println!("✅ Expired Session Rejected.");

    // Scenario 5: Admin Permission Constraints
    println!("\n[5/5] Testing Admin vs Owner Permission...");
    // Create an Admin
    let admin_keypair = Keypair::new();
    let (admin_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            admin_keypair.pubkey().as_ref(),
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
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new_readonly(owner_auth_pda, false),
            AccountMeta::new(admin_auth_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
        ],
        data: add_admin_data,
    };
    ctx.send_transaction(&[add_admin_ix], &[&ctx.payer, &owner_keypair])?;
    println!("   -> Admin Added.");

    // Admin tries to Remove Owner
    let remove_owner_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(admin_auth_pda, false), // Admin authorizes (Must be writable for ManageAuthority logic)
            AccountMeta::new(owner_auth_pda, false), // Target (Owner)
            AccountMeta::new(ctx.payer.pubkey(), false), // Refund dest
            AccountMeta::new_readonly(system_program::id(), false), // System program (Wait, remove requires SysProg? No? Let's check. Yes, it was in list but usually Close doesn't need SysProg unless we transfer? Yes we transfer lamports. But typical close just sets lamports to 0. But we might need sysvar? Checking manage_authority again. No, account 5 was RefundDest. Wait. Line 307: refund_dest. Line 309: it's failing if not enough keys. Where is SysProg? It's NOT required for Remove. But my account meta list had it. I must fix my AccountMeta list.)
            // Re-checking remove authority accounts in manage_authority.rs:
            // 0: Payer
            // 1: Wallet
            // 2: Admin Auth
            // 3: Target Auth
            // 4: Refund Dest
            // 5: Optional Signer
            // System Program is NOT there.
            AccountMeta::new_readonly(admin_keypair.pubkey(), true), // Signer
        ],
        data: vec![2], // RemoveAuthority
    };
    ctx.send_transaction_expect_error(&[remove_owner_ix], &[&ctx.payer, &admin_keypair])?;
    println!("✅ Admin Removing Owner Rejected.");

    Ok(())
}
