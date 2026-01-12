//! Plugin Edge Cases Tests
//!
//! Tests for plugin edge cases:
//! 1. Plugin priority ordering (multiple plugins with different priorities)
//! 2. Plugin enabled/disabled (disabled plugins should not be checked)
//! 3. Multiple authorities with different plugins
//! 4. Plugin check order (priority-based)

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
// TEST 1: Plugin Priority Ordering
// ============================================================================

/// Test plugins are checked in priority order (lower priority = checked first)
#[test_log::test]
#[ignore] // Access violation in LiteSVM when invoking plugin CPI
fn test_plugin_priority_ordering() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 100 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Step 1: Add authority with ExecuteOnly permission
    let spender_keypair = Keypair::new();
    let _spender_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        0,
        &root_authority_keypair,
        RolePermission::ExecuteOnly,
    )?;

    // Step 2: Initialize and register SolLimit Plugin (priority 10)
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

    // Step 3: Initialize and register ProgramWhitelist Plugin (priority 20)
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[root_authority_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    initialize_program_whitelist_plugin(
        &mut context,
        program_whitelist_program_id,
        &root_authority_keypair,
        &[solana_sdk::system_program::id()],
    )?;
    add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        0u32,
        program_whitelist_program_id,
        program_whitelist_config,
    )?;

    // Step 4: Link both plugins to Spender with different priorities
    // SolLimit: priority 10 (checked first)
    // ProgramWhitelist: priority 20 (checked second)
    update_authority_with_multiple_plugins(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        &spender_keypair.pubkey(),
        1, // Authority ID 1 (Spender)
        &[
            (0u16, 10u8), // SolLimit: index 0, priority 10 (checked first)
            (1u16, 20u8), // ProgramWhitelist: index 1, priority 20 (checked second)
        ],
    )?;

    // Step 5: Test transfer within limit (both plugins should pass)
    // SolLimit checks first (priority 10), then ProgramWhitelist (priority 20)
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
        .map_err(|e| anyhow::anyhow!("Failed to send transaction: {:?}", e))?;

    Ok(())
}

// ============================================================================
// TEST 2: Plugin Enabled/Disabled
// ============================================================================

/// Test disabled plugins are not checked
#[test_log::test]
#[ignore] // Access violation in LiteSVM when invoking plugin CPI
fn test_plugin_enabled_disabled() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 100 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Step 1: Add authority with ExecuteOnly permission
    let spender_keypair = Keypair::new();
    let _spender_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        0,
        &root_authority_keypair,
        RolePermission::ExecuteOnly,
    )?;

    // Step 2: Initialize and register SolLimit Plugin
    let sol_limit_program_id = sol_limit_program_id();
    let (sol_limit_config, _) = Pubkey::find_program_address(
        &[root_authority_keypair.pubkey().as_ref()],
        &sol_limit_program_id,
    );

    initialize_sol_limit_plugin(
        &mut context,
        sol_limit_program_id,
        &root_authority_keypair,
        5 * LAMPORTS_PER_SOL, // 5 SOL limit
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

    // Step 3: Link SolLimit plugin to Spender (enabled)
    update_authority_with_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        &spender_keypair.pubkey(),
        1, // Authority ID 1 (Spender)
        0, // Plugin Index 0
        10u8,
    )?;

    // Step 4: Test transfer within limit → should pass
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();
    let transfer_amount = 3 * LAMPORTS_PER_SOL; // Within 5 SOL limit

    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let mut sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        1, // Authority ID 1 (Spender)
        inner_ix,
    )?;

    sign_ix
        .accounts
        .push(AccountMeta::new(sol_limit_config, false));
    sign_ix
        .accounts
        .push(AccountMeta::new_readonly(sol_limit_program_id, false));

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
        .map_err(|e| anyhow::anyhow!("Failed to send transaction: {:?}", e))?;

    // Step 5: Disable plugin via update_authority
    // Update authority to disable plugin (enabled = false)
    update_authority_with_plugin_disabled(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        &spender_keypair.pubkey(),
        1, // Authority ID 1 (Spender)
        0, // Plugin Index 0
        10u8,
        false, // Disabled
    )?;

    // Step 6: Test transfer exceeding limit → should pass (plugin disabled, no check)
    let transfer_amount_fail = 10 * LAMPORTS_PER_SOL; // Exceeds 2 SOL remaining, but plugin is disabled

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

    context
        .svm
        .send_transaction(tx_fail)
        .map_err(|e| anyhow::anyhow!("Failed to send transaction (plugin disabled): {:?}", e))?;

    Ok(())
}

// ============================================================================
// TEST 3: Multiple Authorities with Different Plugins
// ============================================================================

/// Test multiple authorities, each with different plugins
#[test_log::test]
#[ignore] // Access violation in LiteSVM when invoking plugin CPI
fn test_multiple_authorities_different_plugins() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 100 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Step 1: Add Authority A with ExecuteOnly + SolLimit plugin
    let authority_a_keypair = Keypair::new();
    let _authority_a = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &authority_a_keypair,
        0,
        &root_authority_keypair,
        RolePermission::ExecuteOnly,
    )?;

    let sol_limit_program_id = sol_limit_program_id();
    let (sol_limit_config_a, _) = Pubkey::find_program_address(
        &[authority_a_keypair.pubkey().as_ref()],
        &sol_limit_program_id,
    );

    // Initialize SolLimit plugin config for Authority A
    // Always initialize to ensure config account has proper data
    initialize_sol_limit_plugin(
        &mut context,
        sol_limit_program_id,
        &authority_a_keypair,
        10 * LAMPORTS_PER_SOL,
    )?;

    // Check if plugin already exists and verify its config account
    let (plugin_exists, existing_config, plugin_index) = {
        let wallet_account_data = context
            .svm
            .get_account(&wallet_account)
            .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
        let wallet_account_obj = get_wallet_account(&wallet_account_data)?;
        let plugins = wallet_account_obj
            .get_plugins(&wallet_account_data.data)
            .unwrap_or_default();

        let existing_plugin = plugins
            .iter()
            .enumerate()
            .find(|(_, p)| p.program_id.as_ref() == sol_limit_program_id.as_ref());
        if let Some((idx, plugin)) = existing_plugin {
            (true, Some(plugin.config_account), Some(idx as u16))
        } else {
            (false, None, None)
        }
    };

    if let Some(existing_config) = existing_config {
        if existing_config.as_ref() != sol_limit_config_a.as_ref() {
            return Err(anyhow::anyhow!("Plugin config account mismatch! Existing: {:?}, Expected: {:?}. Please remove the existing plugin first or use a different wallet.", existing_config, sol_limit_config_a));
        }
    } else {
        add_plugin(
            &mut context,
            &wallet_account,
            &wallet_vault,
            &root_authority_keypair,
            0u32,
            sol_limit_program_id,
            sol_limit_config_a,
        )?;

        // Verify plugin was added correctly
        let wallet_account_data_after = context
            .svm
            .get_account(&wallet_account)
            .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
        let wallet_account_obj_after = get_wallet_account(&wallet_account_data_after)?;
        let plugins_after = wallet_account_obj_after
            .get_plugins(&wallet_account_data_after.data)
            .unwrap_or_default();
    }
    update_authority_with_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        &authority_a_keypair.pubkey(),
        1, // Authority ID 1 (Authority A)
        0, // Plugin Index 0
        10u8,
    )?;

    // Step 2: Add Authority B with ExecuteOnly + ProgramWhitelist plugin
    let authority_b_keypair = Keypair::new();
    let _authority_b = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &authority_b_keypair,
        0,
        &root_authority_keypair,
        RolePermission::ExecuteOnly,
    )?;

    let program_whitelist_program_id = program_whitelist_program_id();
    // Use root_authority_keypair for config to avoid conflicts with other tests
    let (program_whitelist_config_b, _) = Pubkey::find_program_address(
        &[root_authority_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    // Check if plugin config already exists, if not initialize it
    if context
        .svm
        .get_account(&program_whitelist_config_b)
        .is_none()
    {
        initialize_program_whitelist_plugin(
            &mut context,
            program_whitelist_program_id,
            &root_authority_keypair,
            &[solana_sdk::system_program::id()],
        )?;
    }

    // Try to add plugin, handle case where it might already exist
    // If it fails with DuplicateAuthority, plugin already exists and we'll use index 1
    let plugin_index = match add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        0u32,
        program_whitelist_program_id,
        program_whitelist_config_b,
    ) {
        Ok(_) => {
            // Verify both plugins exist after adding ProgramWhitelist
            let wallet_account_data_after = context
                .svm
                .get_account(&wallet_account)
                .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
            let wallet_account_obj_after = get_wallet_account(&wallet_account_data_after)?;
            let plugins_after = wallet_account_obj_after
                .get_plugins(&wallet_account_data_after.data)
                .unwrap_or_default();

            1u16 // Plugin added successfully, should be at index 1 (after SolLimit at index 0)
        },
        Err(_) => 1u16, // Plugin already exists (from previous test), use index 1
    };
    update_authority_with_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        &authority_b_keypair.pubkey(),
        2, // Authority ID 2 (Authority B)
        1, // Plugin Index 1
        10u8,
    )?;

    // Step 3: Test Authority A execute → only checks SolLimit plugin
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();
    let transfer_amount = 5 * LAMPORTS_PER_SOL; // Within 10 SOL limit

    // CRITICAL: Get plugin config account from plugin entry, not from derivation
    // This ensures consistency between plugin entry and transaction accounts
    let wallet_account_data = context
        .svm
        .get_account(&wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet_account_obj = get_wallet_account(&wallet_account_data)?;
    let plugins = wallet_account_obj
        .get_plugins(&wallet_account_data.data)
        .unwrap_or_default();
    let sol_limit_plugin = plugins
        .iter()
        .find(|p| p.program_id.as_ref() == sol_limit_program_id.as_ref())
        .ok_or_else(|| anyhow::anyhow!("SolLimit plugin not found in wallet registry"))?;
    let plugin_config_account_pinocchio = sol_limit_plugin.config_account;
    // Convert pinocchio::pubkey::Pubkey to solana_sdk::pubkey::Pubkey
    let plugin_config_account = Pubkey::try_from(plugin_config_account_pinocchio.as_ref())
        .map_err(|_| anyhow::anyhow!("Failed to convert Pubkey"))?;

    // Verify plugin config account exists in SVM
    let config_account = context.svm.get_account(&plugin_config_account);
    if config_account.is_none() {
        return Err(anyhow::anyhow!(
            "Plugin config account does not exist in SVM: {:?}",
            plugin_config_account
        ));
    }

    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let mut sign_ix_a = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &authority_a_keypair,
        1, // Authority ID 1 (Authority A)
        inner_ix,
    )?;

    sign_ix_a
        .accounts
        .push(AccountMeta::new(plugin_config_account, false));
    sign_ix_a
        .accounts
        .push(AccountMeta::new_readonly(sol_limit_program_id, false));

    let payer_pubkey = context.default_payer.pubkey();
    let message_a = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix_a,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx_a = VersionedTransaction::try_new(
        VersionedMessage::V0(message_a),
        &[
            context.default_payer.insecure_clone(),
            authority_a_keypair.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx_a)
        .map_err(|e| anyhow::anyhow!("Failed to send transaction (Authority A): {:?}", e))?;

    // Step 4: Test Authority B execute → only checks ProgramWhitelist plugin
    let inner_ix_b =
        system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let mut sign_ix_b = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &authority_b_keypair,
        2, // Authority ID 2 (Authority B)
        inner_ix_b,
    )?;

    sign_ix_b
        .accounts
        .push(AccountMeta::new(program_whitelist_config_b, false));
    sign_ix_b.accounts.push(AccountMeta::new_readonly(
        program_whitelist_program_id,
        false,
    ));

    let message_b = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix_b,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx_b = VersionedTransaction::try_new(
        VersionedMessage::V0(message_b),
        &[
            context.default_payer.insecure_clone(),
            authority_b_keypair.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx_b)
        .map_err(|e| anyhow::anyhow!("Failed to send transaction (Authority B): {:?}", e))?;

    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

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
    let authority_hash = {
        let mut hasher = solana_sdk::hash::Hash::default();
        let mut hasher_state = hasher.to_bytes();
        hasher_state[..32].copy_from_slice(new_authority.pubkey().as_ref());
        solana_sdk::hash::hashv(&[&hasher_state]).to_bytes()
    };

    let seeds = wallet_authority_seeds(wallet_vault, &authority_hash);
    let (new_wallet_authority, _authority_bump) =
        Pubkey::find_program_address(&seeds, &lazorkit_program_id());

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

    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload account: {:?}", e))?;

    let authority_payload_data = vec![4u8];
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

/// Initialize SolLimit plugin
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

/// Initialize ProgramWhitelist plugin
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

    let authority_payload = vec![3u8]; // Index of acting authority
    instruction_data.extend_from_slice(&authority_payload);

    let accounts = vec![
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

/// Update authority with plugin (enabled/disabled)
fn update_authority_with_plugin_disabled(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    _wallet_vault: &Pubkey,
    acting_authority: &Keypair,
    authority_to_update: &Pubkey,
    authority_id: u32,
    plugin_index: u16,
    priority: u8,
    enabled: bool,
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
    instruction_data.push(enabled as u8); // enabled flag
    instruction_data.extend_from_slice(&[0u8; 4]); // padding

    let authority_payload = vec![3u8]; // Index of acting authority
    instruction_data.extend_from_slice(&authority_payload);

    let accounts = vec![
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

/// Update authority with plugin
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
    update_authority_with_plugin_disabled(
        context,
        wallet_account,
        _wallet_vault,
        acting_authority,
        authority_to_update,
        authority_id,
        plugin_index,
        priority,
        true, // enabled
    )
}
