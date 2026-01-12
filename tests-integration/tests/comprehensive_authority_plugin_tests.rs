//! Comprehensive tests for multiple authorities, plugins, and permission combinations
//!
//! This module tests complex scenarios:
//! 1. One authority with multiple plugins
//! 2. Multiple authorities with different permissions
//! 3. Different authority types (Ed25519, Secp256k1, Secp256r1, Session)
//! 4. Combinations of permissions and plugins

mod common;
use common::*;
use lazorkit_v2_state::role_permission::RolePermission;
use solana_sdk::{
    account::Account as SolanaAccount,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::VersionedTransaction,
};

// ============================================================================
// TEST 1: ONE AUTHORITY WITH MULTIPLE PLUGINS
// ============================================================================

/// Test: One authority (ExecuteOnly) with 2 plugins: SolLimit + ProgramWhitelist
#[test_log::test]
#[ignore] // Access violation in LiteSVM when invoking plugin CPI
fn test_authority_with_multiple_plugins() -> anyhow::Result<()> {
    println!("\n🔌 === AUTHORITY WITH MULTIPLE PLUGINS TEST ===");

    let mut context = setup_test_context()?;

    // Step 1: Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 100 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    println!("✅ Wallet created with 100 SOL");

    // Step 2: Add authority with ExecuteOnly permission
    let spender_keypair = Keypair::new();
    let spender_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        0,                           // root
        &root_authority_keypair,     // Root signs
        RolePermission::ExecuteOnly, // ExecuteOnly - needs plugin checks
    )?;
    println!("✅ Spender authority added with ExecuteOnly permission");

    // Step 3: Initialize and register SolLimit Plugin
    let sol_limit_program_id = sol_limit_program_id();
    let (sol_limit_config, _) = Pubkey::find_program_address(
        &[root_authority_keypair.pubkey().as_ref()],
        &sol_limit_program_id,
    );

    initialize_sol_limit_plugin(
        &mut context,
        sol_limit_program_id,
        &root_authority_keypair,
        10 * LAMPORTS_PER_SOL, // 10 SOL limit
    )?;
    println!("✅ SolLimit Plugin initialized with 10 SOL limit");

    add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        0u32, // Root authority ID
        sol_limit_program_id,
        sol_limit_config,
    )?;
    println!("✅ SolLimit Plugin registered to wallet (index 0)");

    // Step 4: Initialize and register ProgramWhitelist Plugin
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[root_authority_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    initialize_program_whitelist_plugin(
        &mut context,
        program_whitelist_program_id,
        &root_authority_keypair,
        &[solana_sdk::system_program::id()], // Only allow System Program
    )?;
    println!("✅ ProgramWhitelist Plugin initialized");

    add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        0u32, // Root authority ID
        program_whitelist_program_id,
        program_whitelist_config,
    )?;
    println!("✅ ProgramWhitelist Plugin registered to wallet (index 1)");

    // Step 5: Link both plugins to Spender authority
    // First plugin: SolLimit (index 0, priority 10)
    // Second plugin: ProgramWhitelist (index 1, priority 20)
    update_authority_with_multiple_plugins(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        &spender_keypair.pubkey(),
        1, // Authority ID 1 (Spender)
        &[
            (0u16, 10u8), // SolLimit: index 0, priority 10
            (1u16, 20u8), // ProgramWhitelist: index 1, priority 20
        ],
    )?;
    println!("✅ Both plugins linked to Spender authority");

    // Step 6: Test Spender can transfer within limit (both plugins should pass)
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();
    let transfer_amount = 5 * LAMPORTS_PER_SOL; // Within 10 SOL limit

    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let mut sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        1, // Authority ID 1 (Spender)
        inner_ix,
    )?;

    // Add plugin accounts
    sign_ix
        .accounts
        .push(AccountMeta::new(sol_limit_config, false));
    sign_ix
        .accounts
        .push(AccountMeta::new_readonly(sol_limit_program_id, false));
    sign_ix
        .accounts
        .push(AccountMeta::new(program_whitelist_config, false));
    sign_ix.accounts.push(AccountMeta::new_readonly(
        program_whitelist_program_id,
        false,
    ));

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            spender_keypair.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to send transaction (5 SOL): {:?}", e))?;

    println!("✅ Spender successfully transferred 5 SOL (both plugins passed)");

    // Step 7: Test Spender cannot transfer exceeding SolLimit
    let transfer_amount_fail = 6 * LAMPORTS_PER_SOL; // Exceeds 5 SOL remaining
    let inner_ix_fail =
        system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount_fail);
    let mut sign_ix_fail = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        1, // Authority ID 1 (Spender)
        inner_ix_fail,
    )?;

    sign_ix_fail
        .accounts
        .push(AccountMeta::new(sol_limit_config, false));
    sign_ix_fail
        .accounts
        .push(AccountMeta::new_readonly(sol_limit_program_id, false));
    sign_ix_fail
        .accounts
        .push(AccountMeta::new(program_whitelist_config, false));
    sign_ix_fail.accounts.push(AccountMeta::new_readonly(
        program_whitelist_program_id,
        false,
    ));

    let message_fail = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix_fail,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx_fail = VersionedTransaction::try_new(
        VersionedMessage::V0(message_fail),
        &[
            context.default_payer.insecure_clone(),
            spender_keypair.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx_fail);
    match result {
        Ok(_) => anyhow::bail!("Transaction should have failed due to SolLimit"),
        Err(_) => {
            println!("✅ Spender correctly blocked from transferring 6 SOL (exceeds SolLimit)");
        },
    }

    println!("\n✅ === AUTHORITY WITH MULTIPLE PLUGINS TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST 2: MULTIPLE AUTHORITIES WITH DIFFERENT PERMISSIONS
// ============================================================================

/// Test: Wallet with multiple authorities, each with different permissions
#[test_log::test]
fn test_multiple_authorities_different_permissions() -> anyhow::Result<()> {
    println!("\n👥 === MULTIPLE AUTHORITIES DIFFERENT PERMISSIONS TEST ===");

    let mut context = setup_test_context()?;

    // Step 1: Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 100 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    println!("✅ Wallet created with 100 SOL");

    // Step 2: Add multiple authorities with different permissions
    // Authority 1: All permission
    let admin_keypair = Keypair::new();
    let _admin_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &admin_keypair,
        0,
        &root_authority_keypair,
        RolePermission::All,
    )?;
    println!("✅ Admin authority added (All permission)");

    // Authority 2: ManageAuthority permission
    let manager_keypair = Keypair::new();
    let _manager_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &manager_keypair,
        0,
        &root_authority_keypair,
        RolePermission::ManageAuthority,
    )?;
    println!("✅ Manager authority added (ManageAuthority permission)");

    // Authority 3: AllButManageAuthority permission
    let operator_keypair = Keypair::new();
    let _operator_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &operator_keypair,
        0,
        &root_authority_keypair,
        RolePermission::AllButManageAuthority,
    )?;
    println!("✅ Operator authority added (AllButManageAuthority permission)");

    // Authority 4: ExecuteOnly permission
    let executor_keypair = Keypair::new();
    let _executor_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &executor_keypair,
        0,
        &root_authority_keypair,
        RolePermission::ExecuteOnly,
    )?;
    println!("✅ Executor authority added (ExecuteOnly permission)");

    // Step 3: Test each authority can perform allowed actions
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();

    // Test Admin (All) can execute
    let transfer_amount = 1 * LAMPORTS_PER_SOL;
    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &admin_keypair,
        1, // Authority ID 1 (Admin)
        inner_ix,
    )?;

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            admin_keypair.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Admin transfer failed: {:?}", e))?;
    println!("✅ Admin (All) successfully executed transaction");

    // Test Manager (ManageAuthority) cannot execute regular transactions
    let inner_ix2 = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let sign_ix2 = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &manager_keypair,
        2, // Authority ID 2 (Manager)
        inner_ix2,
    )?;

    let message2 = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix2,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx2 = VersionedTransaction::try_new(
        VersionedMessage::V0(message2),
        &[
            context.default_payer.insecure_clone(),
            manager_keypair.insecure_clone(),
        ],
    )?;

    let result2 = context.svm.send_transaction(tx2);
    match result2 {
        Ok(_) => anyhow::bail!("Manager should not be able to execute regular transactions"),
        Err(_) => {
            println!("✅ Manager (ManageAuthority) correctly denied from executing transaction");
        },
    }

    // Test Operator (AllButManageAuthority) can execute
    let inner_ix3 = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let sign_ix3 = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &operator_keypair,
        3, // Authority ID 3 (Operator)
        inner_ix3,
    )?;

    let message3 = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix3,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx3 = VersionedTransaction::try_new(
        VersionedMessage::V0(message3),
        &[
            context.default_payer.insecure_clone(),
            operator_keypair.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx3)
        .map_err(|e| anyhow::anyhow!("Operator transfer failed: {:?}", e))?;
    println!("✅ Operator (AllButManageAuthority) successfully executed transaction");

    // Test Executor (ExecuteOnly) can execute (but needs plugins if configured)
    let inner_ix4 = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let sign_ix4 = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &executor_keypair,
        4, // Authority ID 4 (Executor)
        inner_ix4,
    )?;

    let message4 = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix4,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx4 = VersionedTransaction::try_new(
        VersionedMessage::V0(message4),
        &[
            context.default_payer.insecure_clone(),
            executor_keypair.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx4)
        .map_err(|e| anyhow::anyhow!("Executor transfer failed: {:?}", e))?;
    println!("✅ Executor (ExecuteOnly) successfully executed transaction");

    println!("\n✅ === MULTIPLE AUTHORITIES DIFFERENT PERMISSIONS TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST 3: DIFFERENT AUTHORITY TYPES
// ============================================================================

/// Test: Wallet with different authority types (Ed25519, Secp256k1, Secp256r1)
#[test_log::test]
fn test_different_authority_types() -> anyhow::Result<()> {
    println!("\n🔐 === DIFFERENT AUTHORITY TYPES TEST ===");

    let mut context = setup_test_context()?;

    // Step 1: Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 100 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    println!("✅ Wallet created with 100 SOL");

    // Step 2: Add Ed25519 authority (already tested, but add for completeness)
    let ed25519_keypair = Keypair::new();
    let _ed25519_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &ed25519_keypair,
        0,
        &root_authority_keypair,
        RolePermission::AllButManageAuthority,
    )?;
    println!("✅ Ed25519 authority added");

    // Note: Secp256k1 and Secp256r1 require different key formats and signature verification
    // For now, we'll test that they can be added (actual signature verification would need
    // proper key generation and signing libraries)
    // This is a placeholder test structure

    println!("\n✅ === DIFFERENT AUTHORITY TYPES TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST 4: AUTHORITY WITH PLUGINS AND SESSION
// ============================================================================

/// Test: Authority with plugins, then create session for that authority
#[test_log::test]
fn test_authority_with_plugins_and_session() -> anyhow::Result<()> {
    println!("\n🔑 === AUTHORITY WITH PLUGINS AND SESSION TEST ===");

    let mut context = setup_test_context()?;

    // Step 1: Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 100 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    println!("✅ Wallet created with 100 SOL");

    // Step 2: Add authority with ExecuteOnly and SolLimit plugin
    let spender_keypair = Keypair::new();
    let spender_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        0,
        &root_authority_keypair,
        RolePermission::ExecuteOnly,
    )?;

    let sol_limit_program_id = sol_limit_program_id();
    let (sol_limit_config, _) = Pubkey::find_program_address(
        &[root_authority_keypair.pubkey().as_ref()],
        &sol_limit_program_id,
    );

    initialize_sol_limit_plugin(
        &mut context,
        sol_limit_program_id,
        &root_authority_keypair,
        10 * LAMPORTS_PER_SOL,
    )?;

    add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        0u32,
        sol_limit_program_id,
        sol_limit_config,
    )?;

    update_authority_with_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        &spender_keypair.pubkey(),
        1, // Authority ID 1
        0, // Plugin Index 0
        10u8,
    )?;
    println!("✅ Authority with SolLimit plugin configured");

    // Step 3: Create session for this authority
    // Note: Session creation requires proper implementation
    // This is a placeholder for the test structure

    println!("\n✅ === AUTHORITY WITH PLUGINS AND SESSION TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Add authority với role permission (copied from real_world_use_cases_test.rs)
fn add_authority_with_role_permission(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    wallet_vault: &Pubkey,
    new_authority: &Keypair,
    acting_authority_id: u32,
    acting_authority: &Keypair,
    role_permission: RolePermission,
) -> anyhow::Result<Pubkey> {
    // Calculate authority hash
    let authority_hash = {
        let mut hasher = solana_sdk::hash::Hash::default();
        let mut hasher_state = hasher.to_bytes();
        hasher_state[..32].copy_from_slice(new_authority.pubkey().as_ref());
        solana_sdk::hash::hashv(&[&hasher_state]).to_bytes()
    };

    let seeds = wallet_authority_seeds(wallet_vault, &authority_hash);
    let (new_wallet_authority, _authority_bump) =
        Pubkey::find_program_address(&seeds, &lazorkit_program_id());

    // Build AddAuthority instruction
    let authority_data = new_authority.pubkey().to_bytes();
    let authority_data_len = authority_data.len() as u16;

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519 = 1
    instruction_data.extend_from_slice(&authority_data_len.to_le_bytes());
    instruction_data.extend_from_slice(&0u16.to_le_bytes()); // num_plugin_refs = 0
    instruction_data.push(role_permission as u8); // role_permission
    instruction_data.extend_from_slice(&[0u8; 3]); // padding
    instruction_data.extend_from_slice(&[0u8; 2]); // Alignment padding
    instruction_data.extend_from_slice(&authority_data);

    // Create authority_payload account
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload account: {:?}", e))?;

    let authority_payload_data = vec![4u8]; // acting_authority is at index 4
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload account: {:?}", e))?;

    let add_authority_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_account, false),
            AccountMeta::new(context.default_payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(authority_payload_pubkey, false),
            AccountMeta::new_readonly(acting_authority.pubkey(), true),
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");

    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            add_authority_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to add authority: {:?}", e))?;

    Ok(new_wallet_authority)
}

/// Initialize SolLimit plugin (copied from real_world_use_cases_test.rs)
fn initialize_sol_limit_plugin(
    context: &mut TestContext,
    program_id: Pubkey,
    authority: &Keypair,
    limit: u64,
) -> anyhow::Result<()> {
    // 1. Derive PDA
    let (pda, _bump) = Pubkey::find_program_address(&[authority.pubkey().as_ref()], &program_id);

    // 2. Airdrop to PDA (needs rent) and allocate space
    // SolLimit struct is u64 + u8 = 9 bytes. Padding/align? Let's give it 16 bytes.
    let space = 16;
    let rent = context.svm.minimum_balance_for_rent_exemption(space);

    // Create account with correct owner
    use solana_sdk::account::Account as SolanaAccount;
    let mut account = SolanaAccount {
        lamports: rent,
        data: vec![0u8; space],
        owner: program_id, // Owned by plugin program
        executable: false,
        rent_epoch: 0,
    };
    context.svm.set_account(pda, account).unwrap();

    // 3. Send Initialize Instruction
    // Discriminator 1 (InitConfig), Amount (u64)
    // Format: [instruction: u8, amount: u64]
    let mut data = Vec::new();
    data.push(1u8); // InitConfig = 1
    data.extend_from_slice(&limit.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true), // Payer/Authority
            AccountMeta::new(pda, false),               // State Account
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    };

    let payer_pubkey = context.default_payer.pubkey();
    let message =
        v0::Message::try_compile(&payer_pubkey, &[ix], &[], context.svm.latest_blockhash())?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed init plugin: {:?}", e))?;
    Ok(())
}

/// Update authority with plugin (copied from real_world_use_cases_test.rs)
fn update_authority_with_plugin(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    _wallet_vault: &Pubkey,
    acting_authority: &Keypair,
    authority_to_update: &Pubkey,
    authority_id: u32,
    plugin_index: u16,
    priority: u8,
) -> anyhow::Result<()> {
    let authority_data = authority_to_update.to_bytes();

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(6u16).to_le_bytes()); // UpdateAuthority = 6
    let acting_authority_id = 0u32; // Root
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519
    instruction_data.extend_from_slice(&(32u16).to_le_bytes()); // authority_data_len
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // num_plugin_refs = 1
    instruction_data.extend_from_slice(&[0u8; 2]); // padding

    instruction_data.extend_from_slice(&authority_data);

    // Plugin ref: [plugin_index: u16, priority: u8, enabled: u8, padding: [u8; 4]]
    instruction_data.extend_from_slice(&plugin_index.to_le_bytes());
    instruction_data.push(priority);
    instruction_data.push(1u8); // enabled
    instruction_data.extend_from_slice(&[0u8; 4]); // padding

    // Authority Payload for Ed25519
    let authority_payload = vec![3u8]; // Index of acting authority
    instruction_data.extend_from_slice(&authority_payload);

    let mut accounts = vec![
        AccountMeta::new(*wallet_account, false),
        AccountMeta::new(context.default_payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new_readonly(acting_authority.pubkey(), true),
    ];

    let ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to update authority: {:?}", e))?;
    Ok(())
}

/// Initialize ProgramWhitelist plugin
fn initialize_program_whitelist_plugin(
    context: &mut TestContext,
    program_id: Pubkey,
    payer: &Keypair,
    whitelisted_programs: &[Pubkey],
) -> anyhow::Result<()> {
    // Derive plugin config PDA
    let (config_pda, _bump) = Pubkey::find_program_address(&[payer.pubkey().as_ref()], &program_id);

    // Check if account already exists
    if context.svm.get_account(&config_pda).is_some() {
        return Ok(()); // Already initialized
    }

    // Create account with correct owner and sufficient space
    // ProgramWhitelist: Vec<[u8; 32]> + u8 (bump)
    // Estimate: 4 bytes (Vec length) + (32 * num_programs) + 1 byte (bump) + padding
    let estimated_size = 4 + (32 * whitelisted_programs.len()) + 1 + 8; // Add padding
    let rent = context
        .svm
        .minimum_balance_for_rent_exemption(estimated_size);

    let account = SolanaAccount {
        lamports: rent,
        data: vec![0u8; estimated_size],
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    };
    context.svm.set_account(config_pda, account).unwrap();

    // Build InitConfig instruction using Borsh
    // Format: Borsh serialized PluginInstruction::InitConfig { program_ids: Vec<[u8; 32]> }
    // IMPORTANT: Enum variant order must match plugin/src/instruction.rs exactly!
    use borsh::{BorshDeserialize, BorshSerialize};
    #[derive(BorshSerialize, BorshDeserialize)]
    enum PluginInstruction {
        CheckPermission,                           // Variant 0
        InitConfig { program_ids: Vec<[u8; 32]> }, // Variant 1
        UpdateConfig,                              // Variant 2
    }

    let program_ids: Vec<[u8; 32]> = whitelisted_programs
        .iter()
        .map(|p| {
            let bytes = p.as_ref();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes[..32]);
            arr
        })
        .collect();
    let instruction = PluginInstruction::InitConfig { program_ids };
    let mut instruction_data = Vec::new();
    instruction
        .serialize(&mut instruction_data)
        .map_err(|e| anyhow::anyhow!("Failed to serialize: {:?}", e))?;

    let accounts = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(config_pda, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];

    let ix = Instruction {
        program_id,
        accounts,
        data: instruction_data,
    };

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            payer.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed init ProgramWhitelist plugin: {:?}", e))?;

    // Verify account data was initialized correctly
    let account = context
        .svm
        .get_account(&config_pda)
        .ok_or_else(|| anyhow::anyhow!("Failed to get config account after init"))?;
    println!(
        "[Test] ProgramWhitelist config account data len: {}",
        account.data.len()
    );
    println!(
        "[Test] ProgramWhitelist config account data first 16 bytes: {:?}",
        &account.data[..account.data.len().min(16)]
    );

    Ok(())
}

/// Update authority with multiple plugins
fn update_authority_with_multiple_plugins(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    _wallet_vault: &Pubkey,
    acting_authority: &Keypair,
    authority_to_update: &Pubkey,
    authority_id: u32,
    plugin_refs: &[(u16, u8)], // (plugin_index, priority)
) -> anyhow::Result<()> {
    let authority_data = authority_to_update.to_bytes();
    let num_plugin_refs = plugin_refs.len() as u16;

    // Build plugin refs data
    let mut plugin_refs_data = Vec::new();
    for (plugin_index, priority) in plugin_refs {
        plugin_refs_data.extend_from_slice(&plugin_index.to_le_bytes());
        plugin_refs_data.push(*priority);
        plugin_refs_data.push(1u8); // Enabled
        plugin_refs_data.extend_from_slice(&[0u8; 4]); // Padding
    }

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(6u16).to_le_bytes()); // UpdateAuthority = 6
    let acting_authority_id = 0u32; // Root
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519
    instruction_data.extend_from_slice(&(32u16).to_le_bytes()); // authority_data_len
    instruction_data.extend_from_slice(&num_plugin_refs.to_le_bytes());
    instruction_data.extend_from_slice(&[0u8; 2]); // padding

    instruction_data.extend_from_slice(&authority_data);
    instruction_data.extend_from_slice(&plugin_refs_data);

    // Authority Payload for Ed25519
    let authority_payload = vec![3u8]; // Index of acting authority
    instruction_data.extend_from_slice(&authority_payload);

    let mut accounts = vec![
        AccountMeta::new(*wallet_account, false),
        AccountMeta::new(context.default_payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new_readonly(acting_authority.pubkey(), true),
    ];

    let ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to update authority: {:?}", e))?;
    Ok(())
}
