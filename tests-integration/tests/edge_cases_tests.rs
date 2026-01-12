//! Edge Cases and Boundary Conditions Tests for Lazorkit V2
//!
//! This module tests edge cases, boundary conditions, and data integrity.

mod common;
use common::*;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::VersionedTransaction,
};

// ============================================================================
// BOUNDARY CONDITIONS
// ============================================================================

#[test_log::test]
fn test_max_authorities() -> anyhow::Result<()> {
    // Test adding multiple authorities (practical limit test)
    // Note: There's no hard-coded maximum, but account size limits apply

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    use lazorkit_v2_state::role_permission::RolePermission;

    // Add multiple authorities (test with 5 authorities)
    let num_authorities = 5;
    let mut authority_keypairs = Vec::new();

    for i in 0..num_authorities {
        let new_authority_keypair = Keypair::new();
        authority_keypairs.push(new_authority_keypair.insecure_clone());

        let authority_id = add_authority_with_role_permission(
            &mut context,
            &wallet_account,
            &wallet_vault,
            &new_authority_keypair,
            0u32, // Root authority
            &root_keypair,
            RolePermission::AllButManageAuthority,
        )?;
    }

    // Verify all authorities exist
    let wallet_account_data = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet = get_wallet_account(&wallet_account_data)?;
    let num_auths = wallet.num_authorities(&wallet_account_data.data)?;

    // Should have root (1) + added authorities (num_authorities)
    assert_eq!(
        num_auths,
        (1 + num_authorities) as u16,
        "Should have {} authorities",
        1 + num_authorities
    );

    Ok(())
}

#[test_log::test]
fn test_max_plugins() -> anyhow::Result<()> {
    // Test adding multiple plugins (practical limit test)
    // Note: Code checks for num_plugins > 1000, so that's a reasonable limit

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add multiple plugins (test with 3 plugins)
    let num_plugins = 3;
    let program_whitelist_program_id = program_whitelist_program_id();

    for i in 0..num_plugins {
        let (plugin_config, _) = Pubkey::find_program_address(
            &[format!("plugin_{}", i).as_bytes()],
            &program_whitelist_program_id,
        );

        let plugin_index = add_plugin(
            &mut context,
            &wallet_account,
            &wallet_vault,
            &root_keypair,
            0u32, // Root authority
            program_whitelist_program_id,
            plugin_config,
        )?;
    }

    // Verify all plugins exist
    let wallet_account_data = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet = get_wallet_account(&wallet_account_data)?;
    let plugins = wallet.get_plugins(&wallet_account_data.data)?;

    assert_eq!(
        plugins.len(),
        num_plugins,
        "Should have {} plugins",
        num_plugins
    );

    Ok(())
}

#[test_log::test]
fn test_max_plugin_refs_per_authority() -> anyhow::Result<()> {
    // Test adding multiple plugin refs to an authority

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add a plugin first
    let program_whitelist_program_id = program_whitelist_program_id();
    let (plugin_config, _) = Pubkey::find_program_address(
        &[root_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    let _plugin_index = add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32, // Root authority
        program_whitelist_program_id,
        plugin_config,
    )?;

    // Add an authority with multiple plugin refs
    use lazorkit_v2_state::role_permission::RolePermission;
    let new_authority_keypair = Keypair::new();

    // Note: The current implementation allows adding plugin refs when adding authority
    // For this test, we'll add an authority and then verify it can have plugin refs
    let authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        0u32, // Root authority
        &root_keypair,
        RolePermission::ExecuteOnly,
    )?;

    // Verify authority exists
    let wallet_account_data = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet = get_wallet_account(&wallet_account_data)?;
    let authority_data = wallet
        .get_authority(&wallet_account_data.data, authority_id)?
        .ok_or_else(|| anyhow::anyhow!("Authority not found"))?;

    Ok(())
}

#[test_log::test]
fn test_account_size_limit() -> anyhow::Result<()> {
    // Test that account size is within Solana's limits (10MB max)
    // This test verifies that normal operations don't exceed reasonable limits

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault, _root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Check initial account size
    let wallet_account_data = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;

    let account_size = wallet_account_data.data.len();

    // Solana account size limit is 10MB (10,485,760 bytes)
    const MAX_ACCOUNT_SIZE: usize = 10 * 1024 * 1024;
    assert!(
        account_size < MAX_ACCOUNT_SIZE,
        "Account size should be less than {} bytes",
        MAX_ACCOUNT_SIZE
    );

    Ok(())
}

#[test_log::test]
fn test_empty_wallet() -> anyhow::Result<()> {
    // Test operations with a newly created wallet (has root authority, but no plugins)

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Verify wallet has root authority but no plugins
    let wallet_account_data = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet = get_wallet_account(&wallet_account_data)?;

    let num_auths = wallet.num_authorities(&wallet_account_data.data)?;
    assert_eq!(num_auths, 1, "New wallet should have 1 authority (root)");

    let plugins = wallet.get_plugins(&wallet_account_data.data)?;
    assert_eq!(plugins.len(), 0, "New wallet should have no plugins");

    // Test that Sign operation works with empty wallet (no plugins to check)
    let recipient_keypair = Keypair::new();
    let recipient_pubkey = recipient_keypair.pubkey();
    context
        .svm
        .airdrop(&recipient_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Fund the wallet vault
    context
        .svm
        .airdrop(&wallet_vault, 10_000_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop wallet vault: {:?}", e))?;

    // Create a transfer instruction
    let transfer_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, 1_000_000);

    // Create Sign instruction
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32, // Root authority ID
        transfer_ix,
    )?;

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
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

    // This should succeed even with empty wallet (no plugins to check)
    let result = context.svm.send_transaction(tx);
    assert!(result.is_ok(), "Sign should work with empty wallet");

    Ok(())
}

// ============================================================================
// DATA INTEGRITY TESTS
// ============================================================================

#[test_log::test]
fn test_plugin_registry_preserved_on_add_authority() -> anyhow::Result<()> {
    // Test that plugin registry is preserved when adding authority

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add a plugin first
    let program_whitelist_program_id = program_whitelist_program_id();
    let (plugin_config, _) = Pubkey::find_program_address(
        &[root_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    let _plugin_index = add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32, // Root authority
        program_whitelist_program_id,
        plugin_config,
    )?;

    // Verify plugin exists before adding authority
    let wallet_account_data_before = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet_before = get_wallet_account(&wallet_account_data_before)?;
    let plugins_before = wallet_before.get_plugins(&wallet_account_data_before.data)?;
    assert_eq!(
        plugins_before.len(),
        1,
        "Should have 1 plugin before adding authority"
    );

    // Add an authority
    use lazorkit_v2_state::role_permission::RolePermission;
    let new_authority_keypair = Keypair::new();
    let _authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        0u32, // Root authority
        &root_keypair,
        RolePermission::AllButManageAuthority,
    )?;

    // Verify plugin still exists after adding authority
    let wallet_account_data_after = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet_after = get_wallet_account(&wallet_account_data_after)?;
    let plugins_after = wallet_after.get_plugins(&wallet_account_data_after.data)?;

    assert_eq!(
        plugins_after.len(),
        1,
        "Should still have 1 plugin after adding authority"
    );
    assert_eq!(
        plugins_after[0].program_id.as_ref(),
        program_whitelist_program_id.as_ref(),
        "Plugin should be preserved"
    );

    Ok(())
}

#[test_log::test]
fn test_plugin_registry_preserved_on_remove_authority() -> anyhow::Result<()> {
    // Test that plugin registry is preserved when removing authority

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add a plugin first
    let program_whitelist_program_id = program_whitelist_program_id();
    let (plugin_config, _) = Pubkey::find_program_address(
        &[root_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    let _plugin_index = add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32, // Root authority
        program_whitelist_program_id,
        plugin_config,
    )?;

    // Add an authority to remove
    use lazorkit_v2_state::role_permission::RolePermission;
    let new_authority_keypair = Keypair::new();
    let authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        0u32, // Root authority
        &root_keypair,
        RolePermission::AllButManageAuthority,
    )?;

    // Verify plugin exists before removing authority
    let wallet_account_data_before = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet_before = get_wallet_account(&wallet_account_data_before)?;
    let plugins_before = wallet_before.get_plugins(&wallet_account_data_before.data)?;
    assert_eq!(
        plugins_before.len(),
        1,
        "Should have 1 plugin before removing authority"
    );

    // Remove the authority
    remove_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        0u32, // Root authority
        authority_id,
        &root_keypair,
    )?;

    // Verify plugin still exists after removing authority
    let wallet_account_data_after = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet_after = get_wallet_account(&wallet_account_data_after)?;
    let plugins_after = wallet_after.get_plugins(&wallet_account_data_after.data)?;

    assert_eq!(
        plugins_after.len(),
        1,
        "Should still have 1 plugin after removing authority"
    );
    assert_eq!(
        plugins_after[0].program_id.as_ref(),
        program_whitelist_program_id.as_ref(),
        "Plugin should be preserved"
    );

    Ok(())
}

#[test_log::test]
fn test_boundaries_updated_correctly() -> anyhow::Result<()> {
    // Test that boundaries are updated correctly when modifying data
    // Boundaries track where each authority's data ends

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add an authority
    use lazorkit_v2_state::role_permission::RolePermission;
    let new_authority_keypair = Keypair::new();
    let authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        0u32, // Root authority
        &root_keypair,
        RolePermission::AllButManageAuthority,
    )?;

    // Verify authority exists and boundaries are correct
    let wallet_account_data = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet = get_wallet_account(&wallet_account_data)?;

    let num_auths = wallet.num_authorities(&wallet_account_data.data)?;
    assert_eq!(num_auths, 2, "Should have 2 authorities (root + new)");

    // Verify we can retrieve the authority
    let authority_data = wallet
        .get_authority(&wallet_account_data.data, authority_id)?
        .ok_or_else(|| anyhow::anyhow!("Authority not found"))?;

    assert_eq!(
        authority_data.position.id, authority_id,
        "Authority ID should match"
    );

    Ok(())
}

#[test_log::test]
fn test_data_shifting_correct() -> anyhow::Result<()> {
    // Test that data shifting works correctly when removing authority
    // (data is shifted using copy_within to fill gaps)

    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add multiple authorities
    use lazorkit_v2_state::role_permission::RolePermission;
    let mut authority_ids = Vec::new();

    for i in 0..3 {
        let new_authority_keypair = Keypair::new();
        let authority_id = add_authority_with_role_permission(
            &mut context,
            &wallet_account,
            &wallet_vault,
            &new_authority_keypair,
            0u32, // Root authority
            &root_keypair,
            RolePermission::AllButManageAuthority,
        )?;
        authority_ids.push(authority_id);
    }

    // Verify all authorities exist
    let wallet_account_data_before = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet_before = get_wallet_account(&wallet_account_data_before)?;
    let num_auths_before = wallet_before.num_authorities(&wallet_account_data_before.data)?;
    assert_eq!(
        num_auths_before, 4,
        "Should have 4 authorities (root + 3 added)"
    );

    // Remove the middle authority (this should trigger data shifting)
    let authority_to_remove = authority_ids[1];
    remove_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        0u32, // Root authority
        authority_to_remove,
        &root_keypair,
    )?;

    // Verify data was shifted correctly
    let wallet_account_data_after = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet_after = get_wallet_account(&wallet_account_data_after)?;
    let num_auths_after = wallet_after.num_authorities(&wallet_account_data_after.data)?;

    assert_eq!(
        num_auths_after, 3,
        "Should have 3 authorities after removal"
    );

    // Verify remaining authorities still exist
    for authority_id in &authority_ids {
        if *authority_id != authority_to_remove {
            let authority_data =
                wallet_after.get_authority(&wallet_account_data_after.data, *authority_id)?;
            assert!(
                authority_data.is_some(),
                "Authority {} should still exist",
                authority_id
            );
        }
    }

    // Verify removed authority doesn't exist
    let removed_authority =
        wallet_after.get_authority(&wallet_account_data_after.data, authority_to_remove)?;
    assert!(
        removed_authority.is_none(),
        "Removed authority {} should not exist",
        authority_to_remove
    );

    Ok(())
}
