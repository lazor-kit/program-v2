//! Plugin Management Permission Tests
//!
//! Tests that verify only `All` permission can manage plugins:
//! - All: Can add/remove/update plugins ✅
//! - ManageAuthority: Cannot manage plugins ❌
//! - AllButManageAuthority: Cannot manage plugins ❌
//! - ExecuteOnly: Cannot manage plugins ❌

mod common;
use common::*;
use lazorkit_v2_state::role_permission::RolePermission;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};

// ============================================================================
// TEST: All Permission - Can Manage Plugins
// ============================================================================

/// Test All permission can add/remove/update plugins
#[test_log::test]
fn test_all_permission_can_manage_plugins() -> anyhow::Result<()> {
    println!("\n🔓 === ALL PERMISSION CAN MANAGE PLUGINS TEST ===");

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    println!("✅ Wallet created with root authority (All permission)");

    // Test 1: All can add plugin
    let sol_limit_program_id = sol_limit_program_id();
    let (sol_limit_config, _) =
        Pubkey::find_program_address(&[root_keypair.pubkey().as_ref()], &sol_limit_program_id);

    initialize_sol_limit_plugin(
        &mut context,
        sol_limit_program_id,
        &root_keypair,
        10 * LAMPORTS_PER_SOL,
    )?;

    let result = common::add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32, // Root authority ID (All permission)
        sol_limit_program_id,
        sol_limit_config,
    );
    assert!(result.is_ok(), "All permission should allow add_plugin");
    println!("✅ All permission: Can add plugin");

    // Test 2: All can update plugin
    // Note: update_plugin requires UpdatePlugin instruction which needs to be implemented
    // For now, we'll skip this test and focus on add/remove
    // TODO: Implement update_plugin helper when UpdatePlugin instruction is ready
    println!(
        "⚠️  Update plugin test skipped (UpdatePlugin instruction not yet implemented in helpers)"
    );

    // Test 3: All can remove plugin
    // Note: remove_plugin requires RemovePlugin instruction which needs to be implemented
    // For now, we'll skip this test and focus on add
    // TODO: Implement remove_plugin helper when RemovePlugin instruction is ready
    println!(
        "⚠️  Remove plugin test skipped (RemovePlugin instruction not yet implemented in helpers)"
    );

    println!("\n✅ === ALL PERMISSION CAN MANAGE PLUGINS TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST: ManageAuthority Permission - Cannot Manage Plugins
// ============================================================================

/// Test ManageAuthority permission cannot manage plugins
#[test_log::test]
fn test_manage_authority_cannot_manage_plugins() -> anyhow::Result<()> {
    println!("\n👔 === MANAGE AUTHORITY CANNOT MANAGE PLUGINS TEST ===");

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Add authority with ManageAuthority permission
    let admin_keypair = Keypair::new();
    let _admin_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &admin_keypair,
        0,
        &root_keypair,
        RolePermission::ManageAuthority,
    )?;
    println!("✅ Admin authority added with ManageAuthority permission");

    // Initialize plugin first (using root)
    let sol_limit_program_id = sol_limit_program_id();
    let (sol_limit_config, _) =
        Pubkey::find_program_address(&[root_keypair.pubkey().as_ref()], &sol_limit_program_id);

    initialize_sol_limit_plugin(
        &mut context,
        sol_limit_program_id,
        &root_keypair,
        10 * LAMPORTS_PER_SOL,
    )?;

    // Add plugin using root (All permission)
    common::add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32,
        sol_limit_program_id,
        sol_limit_config,
    )?;
    println!("✅ Plugin added by root (All permission)");

    // Test 1: ManageAuthority CANNOT add plugin
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[admin_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    initialize_program_whitelist_plugin(
        &mut context,
        program_whitelist_program_id,
        &admin_keypair,
        &[solana_sdk::system_program::id()],
    )?;

    let result = common::add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &admin_keypair,
        1u32, // Admin authority ID (ManageAuthority - should fail)
        program_whitelist_program_id,
        program_whitelist_config,
    );
    assert!(
        result.is_err(),
        "ManageAuthority should NOT allow add_plugin"
    );
    println!("✅ ManageAuthority: Correctly denied from adding plugin");

    // Test 2: ManageAuthority CANNOT update plugin
    // TODO: Implement update_plugin helper when UpdatePlugin instruction is ready
    println!(
        "⚠️  Update plugin test skipped (UpdatePlugin instruction not yet implemented in helpers)"
    );

    // Test 3: ManageAuthority CANNOT remove plugin
    // TODO: Implement remove_plugin helper when RemovePlugin instruction is ready
    println!(
        "⚠️  Remove plugin test skipped (RemovePlugin instruction not yet implemented in helpers)"
    );

    println!("\n✅ === MANAGE AUTHORITY CANNOT MANAGE PLUGINS TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST: AllButManageAuthority Permission - Cannot Manage Plugins
// ============================================================================

/// Test AllButManageAuthority permission cannot manage plugins
#[test_log::test]
fn test_all_but_manage_authority_cannot_manage_plugins() -> anyhow::Result<()> {
    println!("\n🔒 === ALL BUT MANAGE AUTHORITY CANNOT MANAGE PLUGINS TEST ===");

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Add authority with AllButManageAuthority permission
    let operator_keypair = Keypair::new();
    let _operator_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &operator_keypair,
        0,
        &root_keypair,
        RolePermission::AllButManageAuthority,
    )?;
    println!("✅ Operator authority added with AllButManageAuthority permission");

    // Initialize plugin first (using root)
    let sol_limit_program_id = sol_limit_program_id();
    let (sol_limit_config, _) =
        Pubkey::find_program_address(&[root_keypair.pubkey().as_ref()], &sol_limit_program_id);

    initialize_sol_limit_plugin(
        &mut context,
        sol_limit_program_id,
        &root_keypair,
        10 * LAMPORTS_PER_SOL,
    )?;

    // Add plugin using root (All permission)
    common::add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32,
        sol_limit_program_id,
        sol_limit_config,
    )?;
    println!("✅ Plugin added by root (All permission)");

    // Test 1: AllButManageAuthority CANNOT add plugin
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[operator_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    initialize_program_whitelist_plugin(
        &mut context,
        program_whitelist_program_id,
        &operator_keypair,
        &[solana_sdk::system_program::id()],
    )?;

    let result = common::add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &operator_keypair,
        1u32, // Operator authority ID (AllButManageAuthority - should fail)
        program_whitelist_program_id,
        program_whitelist_config,
    );
    assert!(
        result.is_err(),
        "AllButManageAuthority should NOT allow add_plugin"
    );
    println!("✅ AllButManageAuthority: Correctly denied from adding plugin");

    // Test 2: AllButManageAuthority CANNOT update plugin
    // TODO: Implement update_plugin helper when UpdatePlugin instruction is ready
    println!(
        "⚠️  Update plugin test skipped (UpdatePlugin instruction not yet implemented in helpers)"
    );

    // Test 3: AllButManageAuthority CANNOT remove plugin
    // TODO: Implement remove_plugin helper when RemovePlugin instruction is ready
    println!(
        "⚠️  Remove plugin test skipped (RemovePlugin instruction not yet implemented in helpers)"
    );

    println!("\n✅ === ALL BUT MANAGE AUTHORITY CANNOT MANAGE PLUGINS TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST: ExecuteOnly Permission - Cannot Manage Plugins
// ============================================================================

/// Test ExecuteOnly permission cannot manage plugins
#[test_log::test]
fn test_execute_only_cannot_manage_plugins() -> anyhow::Result<()> {
    println!("\n🔐 === EXECUTE ONLY CANNOT MANAGE PLUGINS TEST ===");

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Add authority with ExecuteOnly permission
    let employee_keypair = Keypair::new();
    let _employee_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &employee_keypair,
        0,
        &root_keypair,
        RolePermission::ExecuteOnly,
    )?;
    println!("✅ Employee authority added with ExecuteOnly permission");

    // Initialize plugin first (using root)
    let sol_limit_program_id = sol_limit_program_id();
    let (sol_limit_config, _) =
        Pubkey::find_program_address(&[root_keypair.pubkey().as_ref()], &sol_limit_program_id);

    initialize_sol_limit_plugin(
        &mut context,
        sol_limit_program_id,
        &root_keypair,
        10 * LAMPORTS_PER_SOL,
    )?;

    // Add plugin using root (All permission)
    common::add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32,
        sol_limit_program_id,
        sol_limit_config,
    )?;
    println!("✅ Plugin added by root (All permission)");

    // Test 1: ExecuteOnly CANNOT add plugin
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[employee_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    initialize_program_whitelist_plugin(
        &mut context,
        program_whitelist_program_id,
        &employee_keypair,
        &[solana_sdk::system_program::id()],
    )?;

    let result = common::add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &employee_keypair,
        1u32, // Employee authority ID (ExecuteOnly - should fail)
        program_whitelist_program_id,
        program_whitelist_config,
    );
    assert!(result.is_err(), "ExecuteOnly should NOT allow add_plugin");
    println!("✅ ExecuteOnly: Correctly denied from adding plugin");

    // Test 2: ExecuteOnly CANNOT update plugin
    // TODO: Implement update_plugin helper when UpdatePlugin instruction is ready
    println!(
        "⚠️  Update plugin test skipped (UpdatePlugin instruction not yet implemented in helpers)"
    );

    // Test 3: ExecuteOnly CANNOT remove plugin
    // TODO: Implement remove_plugin helper when RemovePlugin instruction is ready
    println!(
        "⚠️  Remove plugin test skipped (RemovePlugin instruction not yet implemented in helpers)"
    );

    println!("\n✅ === EXECUTE ONLY CANNOT MANAGE PLUGINS TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Add authority with role permission (copied from comprehensive_authority_plugin_tests.rs)
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

    // Authority Payload for Ed25519
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

/// Initialize SolLimit plugin (copied from comprehensive_authority_plugin_tests.rs)
fn initialize_sol_limit_plugin(
    context: &mut TestContext,
    program_id: Pubkey,
    authority: &Keypair,
    limit: u64,
) -> anyhow::Result<()> {
    let (pda, _bump) = Pubkey::find_program_address(&[authority.pubkey().as_ref()], &program_id);
    let space = 16;
    let rent = context.svm.minimum_balance_for_rent_exemption(space);

    use solana_sdk::account::Account as SolanaAccount;
    let mut account = SolanaAccount {
        lamports: rent,
        data: vec![0u8; space],
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    };
    context.svm.set_account(pda, account).unwrap();

    let mut data = Vec::new();
    data.push(1u8); // InitConfig = 1
    data.extend_from_slice(&limit.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(pda, false),
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

/// Initialize ProgramWhitelist plugin (copied from comprehensive_authority_plugin_tests.rs)
fn initialize_program_whitelist_plugin(
    context: &mut TestContext,
    program_id: Pubkey,
    payer: &Keypair,
    whitelisted_programs: &[Pubkey],
) -> anyhow::Result<()> {
    let (config_pda, _bump) = Pubkey::find_program_address(&[payer.pubkey().as_ref()], &program_id);

    if context.svm.get_account(&config_pda).is_some() {
        return Ok(());
    }

    let estimated_size = 4 + (32 * whitelisted_programs.len()) + 1 + 8;
    let rent = context
        .svm
        .minimum_balance_for_rent_exemption(estimated_size);

    use solana_sdk::account::Account as SolanaAccount;
    let account = SolanaAccount {
        lamports: rent,
        data: vec![0u8; estimated_size],
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    };
    context.svm.set_account(config_pda, account).unwrap();

    use borsh::{BorshDeserialize, BorshSerialize};
    #[derive(BorshSerialize, BorshDeserialize)]
    enum PluginInstruction {
        CheckPermission,
        InitConfig { program_ids: Vec<[u8; 32]> },
        UpdateConfig,
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

    Ok(())
}

// Note: update_plugin and remove_plugin helpers are not yet implemented in common/mod.rs
// These tests focus on add_plugin permission checks for now
