use lazorkit_sdk::{
    advanced::instructions,
    core::connection::SolConnection,
    state::AuthorityType,
    utils::{derive_config_pda, derive_vault_pda, fetch_wallet_info},
};
use solana_program_test::tokio;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

mod common;
use common::TestContext;

//=============================================================================
// Test Helpers
//=============================================================================

/// Create wallet with session authority type
async fn create_wallet_with_session_authority(
    ctx: &TestContext,
    authority_type: AuthorityType,
) -> anyhow::Result<(Pubkey, Pubkey, Keypair)> {
    let wallet_id = Keypair::new().pubkey().to_bytes();
    // LazorKit program ID - must match contract declare_id!
    let program_id = Pubkey::from_str("LazorKit11111111111111111111111111111111111").unwrap();

    let (config_pda, _bump) = derive_config_pda(&program_id, &wallet_id);
    let (vault_pda, _vault_bump) = derive_vault_pda(&program_id, &config_pda);

    let owner_keypair = Keypair::new();

    // Create authority data based on type
    let authority_data = match authority_type {
        AuthorityType::Ed25519Session => {
            let mut data = Vec::new();
            data.extend_from_slice(&owner_keypair.pubkey().to_bytes()); // master_key
            data.extend_from_slice(&[0u8; 32]); // session_key (empty)
            data.extend_from_slice(&3600u64.to_le_bytes()); // max_session_length
            data.extend_from_slice(&0u64.to_le_bytes()); // current_session_expiration
            data
        },
        AuthorityType::Secp256r1Session => {
            // For testing, we'll use a dummy secp256r1 key
            let mut data = Vec::new();
            let dummy_pubkey = [0x02u8; 33]; // Compressed pubkey starts with 02 or 03
            data.extend_from_slice(&dummy_pubkey); // public_key
            data.extend_from_slice(&[0u8; 3]); // padding
            data.extend_from_slice(&0u32.to_le_bytes()); // signature_odometer
            data.extend_from_slice(&[0u8; 32]); // session_key (empty)
            data.extend_from_slice(&86400u64.to_le_bytes()); // max_session_age
            data.extend_from_slice(&0u64.to_le_bytes()); // current_session_expiration
            data
        },
        _ => panic!("Invalid session authority type"),
    };

    let ix = instructions::create_wallet(
        &program_id,
        &ctx.payer.pubkey(),
        wallet_id,
        authority_type,
        authority_data,
    );

    let recent_blockhash = ctx.get_latest_blockhash().await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        recent_blockhash,
    );

    ctx.send_transaction(&tx)
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    Ok((config_pda, vault_pda, owner_keypair))
}

/// Create session for a role
async fn create_session_for_role(
    ctx: &TestContext,
    config_pda: &Pubkey,
    role_id: u32,
    duration: u64,
    master_keypair: &Keypair,
) -> anyhow::Result<Keypair> {
    let session_keypair = Keypair::new();
    let program_id = Pubkey::from_str("LazorKit11111111111111111111111111111111111").unwrap();

    let ix = instructions::create_session(
        &program_id,
        config_pda,
        &ctx.payer.pubkey(),
        role_id,
        session_keypair.pubkey().to_bytes(),
        duration,
        Vec::new(), // authorization_data (empty for Ed25519)
        vec![solana_sdk::instruction::AccountMeta::new_readonly(
            master_keypair.pubkey(),
            true,
        )],
    );

    let recent_blockhash = ctx.get_latest_blockhash().await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, master_keypair],
        recent_blockhash,
    );

    ctx.send_transaction(&tx)
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    Ok(session_keypair)
}

/// Verify session is active for a role
async fn verify_session_active(
    ctx: &TestContext,
    config_pda: &Pubkey,
    role_id: u32,
) -> anyhow::Result<bool> {
    let wallet_info = fetch_wallet_info(ctx, config_pda).await?;

    if let Some(role) = wallet_info.roles.iter().find(|r| r.id == role_id) {
        if let Some(expiration) = role.current_session_expiration {
            // Get current slot (simplified - use banks client for real slot)
            return Ok(expiration > 0);
        }
    }

    Ok(false)
}

/// Advance to specific slot (test helper)
async fn advance_slots(ctx: &TestContext, slots: u64) {
    // In solana-program-test, we can warp to slots
    let mut context = ctx.context.lock().await;
    let current_slot = context.banks_client.get_root_slot().await.unwrap();
    context.warp_to_slot(current_slot + slots).unwrap();
}

/// Execute instruction with session
async fn execute_with_session(
    ctx: &TestContext,
    config_pda: &Pubkey,
    role_id: u32,
    session_keypair: &Keypair,
    payload: &[u8],
    account_metas: Vec<solana_sdk::instruction::AccountMeta>,
    _program_id: &Pubkey,
) -> anyhow::Result<()> {
    // Auth payload: Index of the session key account.
    // Accounts structure:
    // 0: Config
    // 1: Vault
    // 2: System
    // 3: Session Key (We inject this)
    // 4+: Target accounts

    // Inject Session Key at the start of account_metas
    let mut modified_accounts = account_metas;
    modified_accounts.insert(
        0,
        solana_sdk::instruction::AccountMeta::new_readonly(
            session_keypair.pubkey(),
            true, // is_signer
        ),
    );

    // Auth payload: Index of the session key account (which is at index 3: Config, Vault, System, Session)
    let auth_payload = vec![3u8];
    // execution_payload is just the target data
    let execution_payload = payload.to_vec();

    let lazorkit_program_id =
        Pubkey::from_str("LazorKit11111111111111111111111111111111111").unwrap();
    let (vault_pda, _) = derive_vault_pda(&lazorkit_program_id, config_pda);

    let ix = instructions::execute(
        &lazorkit_program_id,
        config_pda,
        &vault_pda,
        role_id,
        execution_payload,
        auth_payload,
        modified_accounts,
    );

    let recent_blockhash = ctx.get_latest_blockhash().await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, session_keypair],
        recent_blockhash,
    );

    ctx.send_transaction(&tx)
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    Ok(())
}

#[tokio::test]
async fn test_invalid_session_rejected() {
    let ctx = TestContext::new().await;

    let (config_pda, _vault, owner_keypair) =
        create_wallet_with_session_authority(&ctx, AuthorityType::Ed25519Session)
            .await
            .expect("Failed to create wallet");

    // Create valid session
    let _valid_session = create_session_for_role(&ctx, &config_pda, 0, 3600, &owner_keypair)
        .await
        .expect("Failed to create session");

    // Try to execute with DIFFERENT (invalid) session key
    let invalid_session = Keypair::new();

    let transfer_ix = solana_sdk::system_instruction::transfer(
        &ctx.context.lock().await.payer.pubkey(),
        &Keypair::new().pubkey(),
        1_000_000,
    );

    let execute_ix = instructions::execute(
        &Pubkey::from_str("LazorKit11111111111111111111111111111111111").unwrap(),
        &config_pda,
        &_vault,
        0,                        // role_id
        transfer_ix.data.clone(), // Instruction Data
        vec![3],                  // Auth Data (Index of invalid session key)
        vec![
            solana_sdk::instruction::AccountMeta::new(ctx.payer.pubkey(), false),
            solana_sdk::instruction::AccountMeta::new(Keypair::new().pubkey(), false),
            solana_sdk::instruction::AccountMeta::new_readonly(
                solana_sdk::system_program::id(),
                false,
            ),
            solana_sdk::instruction::AccountMeta::new_readonly(
                invalid_session.pubkey(), // Wrong session key!
                true,
            ),
        ],
    );

    let recent_blockhash = ctx.get_latest_blockhash().await;
    let tx = Transaction::new_signed_with_payer(
        &[execute_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, &invalid_session], // Sign with invalid session
        recent_blockhash,
    );

    let result = ctx.send_transaction(&tx).await;

    // Should fail because session key doesn't match
    assert!(
        result.is_err(),
        "Should reject transaction with invalid session key"
    );
}

//=============================================================================
// P1: Session Lifecycle Tests
//=============================================================================

#[tokio::test]
async fn test_session_lifecycle_complete() {
    let ctx = TestContext::new().await;

    let (config_pda, _vault, owner_keypair) =
        create_wallet_with_session_authority(&ctx, AuthorityType::Ed25519Session)
            .await
            .expect("Failed to create wallet");

    // 1. Create session
    let session1 = create_session_for_role(&ctx, &config_pda, 0, 100, &owner_keypair)
        .await
        .expect("First session creation failed");

    let wallet_info = fetch_wallet_info(&ctx, &config_pda).await.unwrap();
    let owner_role = wallet_info.roles.iter().find(|r| r.id == 0).unwrap();
    assert_eq!(
        owner_role.session_key.unwrap(),
        session1.pubkey().to_bytes()
    );

    // 2. Use session (verified by session key being set)
    assert!(owner_role.current_session_expiration.is_some());

    // 3. Session expires
    advance_slots(&ctx, 150).await;

    // 4. Create new session (should overwrite old one)
    let session2 = create_session_for_role(&ctx, &config_pda, 0, 200, &owner_keypair)
        .await
        .expect("Second session creation failed");

    let wallet_info2 = fetch_wallet_info(&ctx, &config_pda).await.unwrap();
    let owner_role2 = wallet_info2.roles.iter().find(|r| r.id == 0).unwrap();

    assert_eq!(
        owner_role2.session_key.unwrap(),
        session2.pubkey().to_bytes(),
        "New session should replace old one"
    );

    assert_ne!(
        session1.pubkey().to_bytes(),
        session2.pubkey().to_bytes(),
        "Sessions should be different"
    );
}

//=============================================================================
// P1: High Priority Tests (Implemented)
//=============================================================================

#[tokio::test]
async fn test_master_key_management() {
    let ctx = TestContext::new().await;

    // Create wallet with Owner (Ed25519Session)
    let (config_pda, _vault, owner_keypair) =
        create_wallet_with_session_authority(&ctx, AuthorityType::Ed25519Session)
            .await
            .expect("Failed to create wallet");

    // Helper to get role
    let get_owner_role = |wallet: &lazorkit_sdk::types::WalletInfo| {
        wallet.roles.iter().find(|r| r.id == 0).unwrap().clone()
    };

    let wallet_info = fetch_wallet_info(&ctx, &config_pda).await.unwrap();
    let initial_role = get_owner_role(&wallet_info);
    let initial_max_len = initial_role.max_session_length.unwrap();

    assert_eq!(
        initial_max_len, 3600,
        "Initial max session duration should be 3600"
    );

    // Update Master Key settings (max_session_length)
    let mut new_auth_data = Vec::new();
    new_auth_data.extend_from_slice(&owner_keypair.pubkey().to_bytes()); // keep master key
    new_auth_data.extend_from_slice(&initial_role.session_key.unwrap()); // keep session key
    new_auth_data.extend_from_slice(&7200u64.to_le_bytes()); // NEW duration (2 hours)
    new_auth_data.extend_from_slice(&0u64.to_le_bytes());

    let program_id = Pubkey::from_str("LazorKit11111111111111111111111111111111111").unwrap();

    // Transfer Ownership to update params (since UpdateAuthority forbids self-update on Role 0)
    let transfer_ix = instructions::transfer_ownership(
        &program_id,
        &config_pda,
        &owner_keypair.pubkey(),
        2, // AuthorityType::Ed25519Session (raw value because helper takes u16? No, verify helper sig)
        // Helper takes u16? Let's check instructions.rs. Yes u16.
        // AuthorityType::Ed25519Session is 2.
        new_auth_data,
        vec![1], // Auth payload for Ed25519 transfer: [index]. Signer is owner_keypair.
                 // Accounts: Config(0), CurrentOwner(1).
    );

    let recent_blockhash = ctx.get_latest_blockhash().await;
    let tx = Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, &owner_keypair],
        recent_blockhash,
    );

    ctx.send_transaction(&tx)
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))
        .expect("Transfer ownership failed");

    // Verify update
    let wallet_info = fetch_wallet_info(&ctx, &config_pda).await.unwrap();
    let updated_role = get_owner_role(&wallet_info);

    assert_eq!(
        updated_role.max_session_length.unwrap(),
        7200,
        "Max session length should be updated"
    );
}

#[tokio::test]
async fn test_multiple_roles_with_sessions() {
    let ctx = TestContext::new().await;

    // Create wallet Owner
    let (config_pda, _vault, owner_keypair) =
        create_wallet_with_session_authority(&ctx, AuthorityType::Ed25519Session)
            .await
            .expect("Failed to create wallet");

    // Add Admin (Role 1)
    let admin_keypair = Keypair::new();
    let mut admin_auth_data = Vec::new();
    admin_auth_data.extend_from_slice(&admin_keypair.pubkey().to_bytes());
    admin_auth_data.extend_from_slice(&[0u8; 32]);
    admin_auth_data.extend_from_slice(&3600u64.to_le_bytes());
    admin_auth_data.extend_from_slice(&0u64.to_le_bytes());

    let program_id = Pubkey::from_str("LazorKit11111111111111111111111111111111111").unwrap();

    let add_admin = instructions::add_authority(
        &program_id,
        &config_pda,
        &ctx.payer.pubkey(),
        0, // Owner adds Admin
        AuthorityType::Ed25519Session,
        admin_auth_data,
        vec![3],
        vec![solana_sdk::instruction::AccountMeta::new_readonly(
            owner_keypair.pubkey(),
            true,
        )],
    );

    let recent_blockhash = ctx.get_latest_blockhash().await;
    let tx = Transaction::new_signed_with_payer(
        &[add_admin],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, &owner_keypair],
        recent_blockhash,
    );
    ctx.send_transaction(&tx)
        .await
        .expect("Failed to add admin");

    // Create session for Admin (Role 1)
    let admin_session = create_session_for_role(&ctx, &config_pda, 1, 3600, &admin_keypair)
        .await
        .expect("Admin session creation failed");

    // Admin should be able to Execute using SESSION key
    // We send a small SOL transfer using System Program
    let ix = solana_sdk::system_instruction::transfer(
        &ctx.payer.pubkey(), // Does not matter what instruction is, actually.
        &config_pda,
        1,
    );
    // Wait, execute runs `ix` with `dispatch_invoke_signed`.
    // The `ix` must be constructed such that accounts match what `execute` passes.
    // In `execute_with_session` helper:
    // It creates an instruction payload.
    // We should use `execute_with_session` helper.

    // We need to provide correct `account_metas` for the target instruction.
    // Let's do a self-transfer (0 SOL) just to test execution permission.
    let target_ix = solana_sdk::system_instruction::transfer(
        &ctx.payer.pubkey(), // source
        &ctx.payer.pubkey(), // dest
        0,
    );

    // Authorization for Execute:
    // Need Payer and System Program in `accounts` of execute.
    // `execute_with_session` helper handles this.
    // It passes `account_metas` for the target instruction.

    let account_metas = target_ix.accounts.clone();

    execute_with_session(
        &ctx,
        &config_pda,
        1,              // Admin Role
        &admin_session, // Session Key
        &target_ix.data,
        account_metas,
        &target_ix.program_id,
    )
    .await
    .expect("Admin session execution failed");
}
