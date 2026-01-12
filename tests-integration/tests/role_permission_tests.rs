//! Comprehensive Role Permission Tests for Lazorkit V2
//!
//! Tests all 4 role permissions with all functions:
//! - All: Can execute and manage authorities
//! - AllButManageAuthority: Can execute but cannot manage authorities
//! - ExecuteOnly: Can only execute, cannot manage authorities
//! - ManageAuthority: Can only manage authorities, cannot execute

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
    system_instruction,
    transaction::VersionedTransaction,
};

// ============================================================================
// TEST: All Permission
// ============================================================================

/// Test All permission: Can execute transactions and manage authorities
#[test_log::test]
fn test_all_permission() -> anyhow::Result<()> {
    println!("\n🔓 === ALL PERMISSION TEST ===");

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Root authority should have All permission (default)
    println!("✅ Wallet created with root authority (All permission)");

    // Test 1: All can execute transactions (bypass CPI check)
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();
    let transfer_amount = 1 * LAMPORTS_PER_SOL;
    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);

    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0, // Root authority ID
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
            root_keypair.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("All permission should allow execute: {:?}", e))?;
    println!("✅ All permission: Can execute transactions (bypass CPI check)");

    // Test 2: All can add authority
    let new_authority_keypair = Keypair::new();
    let result = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        0, // Root acting
        &root_keypair,
        RolePermission::ExecuteOnly,
    );
    assert!(result.is_ok(), "All permission should allow add_authority");
    println!("✅ All permission: Can add authority");

    // Test 3: All can remove authority
    // Get the authority ID we just added (should be ID 1)
    let wallet_account_data = context.svm.get_account(&wallet_account).unwrap();
    let wallet_account_struct = get_wallet_account(&wallet_account_data)?;
    let num_authorities = wallet_account_struct.num_authorities(&wallet_account_data.data)?;
    assert_eq!(num_authorities, 2, "Should have 2 authorities");

    // Debug: Print all authority IDs
    let mut authority_ids = Vec::new();
    for i in 0..num_authorities {
        if let Ok(Some(auth_data)) =
            wallet_account_struct.get_authority(&wallet_account_data.data, i as u32)
        {
            authority_ids.push(auth_data.position.id);
            println!("  Authority {}: ID = {}", i, auth_data.position.id);
        }
    }
    println!("All authority IDs: {:?}", authority_ids);

    // Remove authority ID 1 (the one we just added)
    let authority_id_to_remove = authority_ids
        .iter()
        .find(|&&id| id != 0)
        .copied()
        .unwrap_or(1);
    println!("Removing authority ID: {}", authority_id_to_remove);
    let remove_result = remove_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        0,
        authority_id_to_remove,
        &root_keypair,
    );
    if let Err(e) = &remove_result {
        println!("❌ Remove authority failed: {:?}", e);
    }
    assert!(
        remove_result.is_ok(),
        "All permission should allow remove_authority"
    );
    println!("✅ All permission: Can remove authority");

    println!("\n✅ === ALL PERMISSION TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST: AllButManageAuthority Permission
// ============================================================================

/// Test AllButManageAuthority: Can execute but cannot manage authorities
#[test_log::test]
fn test_all_but_manage_authority_permission() -> anyhow::Result<()> {
    println!("\n🔒 === ALL BUT MANAGE AUTHORITY PERMISSION TEST ===");

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Add authority with AllButManageAuthority permission
    let manager_keypair = Keypair::new();
    let result = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &manager_keypair,
        0, // Root acting
        &root_keypair,
        RolePermission::AllButManageAuthority,
    )?;
    println!("✅ Manager authority added with AllButManageAuthority permission");

    // Test 1: AllButManageAuthority can execute transactions (bypass CPI check)
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();
    let transfer_amount = 1 * LAMPORTS_PER_SOL;
    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);

    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &manager_keypair,
        1, // Manager authority ID
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
            manager_keypair.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("AllButManageAuthority should allow execute: {:?}", e))?;
    println!("✅ AllButManageAuthority: Can execute transactions (bypass CPI check)");

    // Test 2: AllButManageAuthority CANNOT add authority
    let new_authority_keypair = Keypair::new();
    let result = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        1, // Manager acting (should fail)
        &manager_keypair,
        RolePermission::ExecuteOnly,
    );
    assert!(
        result.is_err(),
        "AllButManageAuthority should NOT allow add_authority"
    );
    println!("✅ AllButManageAuthority: Correctly denied from adding authority");

    // Test 3: AllButManageAuthority CANNOT remove authority
    let remove_result = remove_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        1,
        0,
        &manager_keypair,
    );
    assert!(
        remove_result.is_err(),
        "AllButManageAuthority should NOT allow remove_authority"
    );
    println!("✅ AllButManageAuthority: Correctly denied from removing authority");

    // Test 4: AllButManageAuthority CANNOT update authority
    let update_result = update_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        1, // Manager acting (should fail)
        &manager_keypair,
    );
    assert!(
        update_result.is_err(),
        "AllButManageAuthority should NOT allow update_authority"
    );
    println!("✅ AllButManageAuthority: Correctly denied from updating authority");

    println!("\n✅ === ALL BUT MANAGE AUTHORITY PERMISSION TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST: ExecuteOnly Permission
// ============================================================================

/// Test ExecuteOnly: Can only execute, cannot manage authorities
#[test_log::test]
fn test_execute_only_permission() -> anyhow::Result<()> {
    println!("\n🔐 === EXECUTE ONLY PERMISSION TEST ===");

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
    let result = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &employee_keypair,
        0, // Root acting
        &root_keypair,
        RolePermission::ExecuteOnly,
    )?;
    println!("✅ Employee authority added with ExecuteOnly permission");

    // Test 1: ExecuteOnly can execute transactions (MUST check CPI plugins)
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();
    let transfer_amount = 1 * LAMPORTS_PER_SOL;
    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);

    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &employee_keypair,
        1, // Employee authority ID
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
            employee_keypair.insecure_clone(),
        ],
    )?;

    // ExecuteOnly should work even without plugins (no plugins = no checks needed)
    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("ExecuteOnly should allow execute: {:?}", e))?;
    println!("✅ ExecuteOnly: Can execute transactions (must check plugins if any)");

    // Test 2: ExecuteOnly CANNOT add authority
    let new_authority_keypair = Keypair::new();
    let result = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        1, // Employee acting (should fail)
        &employee_keypair,
        RolePermission::ExecuteOnly,
    );
    assert!(
        result.is_err(),
        "ExecuteOnly should NOT allow add_authority"
    );
    println!("✅ ExecuteOnly: Correctly denied from adding authority");

    // Test 3: ExecuteOnly CANNOT remove authority
    let remove_result = remove_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        1,
        0,
        &employee_keypair,
    );
    assert!(
        remove_result.is_err(),
        "ExecuteOnly should NOT allow remove_authority"
    );
    println!("✅ ExecuteOnly: Correctly denied from removing authority");

    // Test 4: ExecuteOnly CANNOT update authority
    let update_result = update_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        1, // Employee acting (should fail)
        &employee_keypair,
    );
    assert!(
        update_result.is_err(),
        "ExecuteOnly should NOT allow update_authority"
    );
    println!("✅ ExecuteOnly: Correctly denied from updating authority");

    println!("\n✅ === EXECUTE ONLY PERMISSION TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// TEST: ManageAuthority Permission
// ============================================================================

/// Test ManageAuthority: Can only manage authorities, cannot execute
#[test_log::test]
fn test_manage_authority_permission() -> anyhow::Result<()> {
    println!("\n👔 === MANAGE AUTHORITY PERMISSION TEST ===");

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
    let result = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &admin_keypair,
        0, // Root acting
        &root_keypair,
        RolePermission::ManageAuthority,
    )?;
    println!("✅ Admin authority added with ManageAuthority permission");

    // Test 1: ManageAuthority CANNOT execute transactions
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();
    let transfer_amount = 1 * LAMPORTS_PER_SOL;
    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);

    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &admin_keypair,
        1, // Admin authority ID
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

    let result = context.svm.send_transaction(tx);
    assert!(
        result.is_err(),
        "ManageAuthority should NOT allow execute transactions"
    );
    println!("✅ ManageAuthority: Correctly denied from executing transactions");

    // Test 2: ManageAuthority CAN add authority
    let new_authority_keypair = Keypair::new();
    let result = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        1, // Admin acting
        &admin_keypair,
        RolePermission::ExecuteOnly,
    );
    assert!(result.is_ok(), "ManageAuthority should allow add_authority");
    println!("✅ ManageAuthority: Can add authority");

    // Test 3: ManageAuthority CAN remove authority
    // Get the authority ID we just added (should be ID 2)
    let wallet_account_data = context.svm.get_account(&wallet_account).unwrap();
    let wallet_account_struct = get_wallet_account(&wallet_account_data)?;
    let num_authorities = wallet_account_struct.num_authorities(&wallet_account_data.data)?;
    assert_eq!(
        num_authorities, 3,
        "Should have 3 authorities (root + admin + new)"
    );

    let remove_result = remove_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        1,
        2,
        &admin_keypair,
    );
    assert!(
        remove_result.is_ok(),
        "ManageAuthority should allow remove_authority"
    );
    println!("✅ ManageAuthority: Can remove authority");

    // Test 4: ManageAuthority CAN update authority
    // Add another authority first to update
    let update_target_keypair = Keypair::new();
    let _ = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &update_target_keypair,
        1, // Admin acting
        &admin_keypair,
        RolePermission::ExecuteOnly,
    )?;

    let update_result = update_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        1, // Admin acting
        &admin_keypair,
    );
    if let Err(e) = &update_result {
        println!("❌ Update authority failed: {:?}", e);
    }
    assert!(
        update_result.is_ok(),
        "ManageAuthority should allow update_authority"
    );
    println!("✅ ManageAuthority: Can update authority");

    println!("\n✅ === MANAGE AUTHORITY PERMISSION TEST PASSED ===\n");
    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Remove authority helper
fn remove_authority(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    wallet_vault: &Pubkey,
    acting_authority_id: u32,
    authority_id_to_remove: u32,
    acting_authority: &Keypair,
) -> anyhow::Result<()> {
    // Build RemoveAuthority instruction
    // Format: [instruction: u16, acting_authority_id: u32, authority_id: u32]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(7u16).to_le_bytes()); // RemoveAuthority = 7
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&authority_id_to_remove.to_le_bytes());

    // Authority payload for Ed25519
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload: {:?}", e))?;

    let authority_payload_data = vec![4u8]; // acting_authority is at index 4
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload: {:?}", e))?;

    let remove_ix = Instruction {
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

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            remove_ix,
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
        .map_err(|e| anyhow::anyhow!("Failed to remove authority: {:?}", e))?;
    Ok(())
}

/// Update authority helper
fn update_authority(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    wallet_vault: &Pubkey,
    acting_authority_id: u32,
    acting_authority: &Keypair,
) -> anyhow::Result<()> {
    // Get current authority to update (use authority_id = 2 if exists, else 1)
    let wallet_account_data = context.svm.get_account(&wallet_account).unwrap();
    let wallet_account_struct = get_wallet_account(&wallet_account_data)?;
    let num_authorities = wallet_account_struct.num_authorities(&wallet_account_data.data)?;

    // UpdateAuthority allows self-update, so we update the acting authority itself
    let authority_id_to_update = acting_authority_id;

    // Build UpdateAuthority instruction
    // Format: [instruction: u16, acting_authority_id: u32, authority_id: u32,
    //          new_authority_type: u16, new_authority_data_len: u16, num_plugin_refs: u16,
    //          padding: [u8; 2], authority_data]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(6u16).to_le_bytes()); // UpdateAuthority = 6
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes()); // acting_authority_id
    instruction_data.extend_from_slice(&authority_id_to_update.to_le_bytes()); // authority_id to update
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519 = 1
    instruction_data.extend_from_slice(&(32u16).to_le_bytes()); // 32 bytes for Ed25519
    instruction_data.extend_from_slice(&(0u16).to_le_bytes()); // num_plugin_refs = 0
    instruction_data.extend_from_slice(&[0u8; 2]); // padding (2 bytes)
                                                   // UpdateAuthorityArgs is 16 bytes (8 bytes for u32s + 6 bytes for u16s + 2 bytes padding)

    // Get current authority data to keep it the same (self-update)
    if let Ok(Some(acting_auth_data)) =
        wallet_account_struct.get_authority(&wallet_account_data.data, acting_authority_id)
    {
        instruction_data.extend_from_slice(&acting_auth_data.authority_data);
    } else {
        return Err(anyhow::anyhow!("Acting authority not found"));
    }

    // Authority payload for Ed25519
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload: {:?}", e))?;

    let authority_payload_data = vec![4u8]; // acting_authority is at index 4
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload: {:?}", e))?;

    let update_ix = Instruction {
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

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            update_ix,
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

/// Add authority with role permission
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

    let seeds = common::wallet_authority_seeds(wallet_vault, &authority_hash);
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
    instruction_data.extend_from_slice(&[0u8; 3]); // padding (3 bytes)
    instruction_data.extend_from_slice(&[0u8; 2]); // Alignment padding to reach 16 bytes
    instruction_data.extend_from_slice(&authority_data);

    // Authority payload for Ed25519
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload: {:?}", e))?;

    let authority_payload_data = vec![4u8]; // acting_authority is at index 4
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload: {:?}", e))?;

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
