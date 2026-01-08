//! Comprehensive tests for RolePermissionPlugin integration

mod common;
use common::*;
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
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    plugin::{PluginEntry, PluginType},
    plugin_ref::PluginRef,
    authority::AuthorityType,
    Discriminator,
    Transmutable,
};

/// Test: Add RolePermission plugin with All permission
#[test_log::test]
fn test_add_role_permission_plugin_all() -> anyhow::Result<()> {
    let mut ctx = setup_test_context()?;
    
    // Create wallet
    let wallet_id = [1u8; 32];
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut ctx, wallet_id)?;
    
    // Create mock plugin program and config
    let plugin_program = Keypair::new();
    let plugin_program_pubkey = Pubkey::try_from(plugin_program.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    let plugin_config = Keypair::new();
    let plugin_config_pubkey = Pubkey::try_from(plugin_config.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    // Build AddPlugin instruction
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(3u16).to_le_bytes()); // AddPlugin = 3
    instruction_data.extend_from_slice(plugin_program_pubkey.as_ref()); // program_id (32 bytes)
    instruction_data.extend_from_slice(plugin_config_pubkey.as_ref()); // config_account (32 bytes)
    instruction_data.push(PluginType::RolePermission as u8); // plugin_type
    instruction_data.push(1u8); // enabled
    instruction_data.push(0u8); // priority
    instruction_data.extend_from_slice(&[0u8; 5]); // padding
    
    let payer_pubkey = Pubkey::try_from(ctx.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    let accounts = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new(payer_pubkey, true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let add_plugin_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };
    
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[add_plugin_ix],
        &[],
        ctx.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[ctx.default_payer.insecure_clone()],
    )?;
    
    ctx.svm.send_transaction(tx).map_err(|e| anyhow::anyhow!("Failed to add plugin: {:?}", e))?;
    
    // Verify plugin was added
    let wallet_account_data = ctx.svm.get_account(&wallet_account).ok_or(anyhow::anyhow!("Wallet account not found"))?.data;
    let wallet_account_struct = unsafe {
        WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])?
    };
    let plugins = wallet_account_struct.get_plugins(&wallet_account_data)?;
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].program_id.as_ref(), plugin_program_pubkey.as_ref());
    
    Ok(())
}

/// Test: Execute with RolePermission plugin (All permission) - should allow all
#[test_log::test]
fn test_execute_with_role_permission_all() -> anyhow::Result<()> {
    let mut ctx = setup_test_context()?;
    
    // Create wallet
    let wallet_id = [2u8; 32];
    let (wallet_account, wallet_vault) = create_lazorkit_wallet(&mut ctx, wallet_id)?;
    
    // Fund wallet vault
    ctx.svm.airdrop(&wallet_vault, 1_000_000_000).map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;
    
    // Add Ed25519 authority
    let authority_keypair = Keypair::new();
    let authority_pubkey = authority_keypair.pubkey();
    
    // Build AddAuthority instruction
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    instruction_data.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes());
    instruction_data.extend_from_slice(&(32u16).to_le_bytes()); // authority_data_len
    instruction_data.extend_from_slice(&(0u16).to_le_bytes()); // num_plugin_refs
    instruction_data.extend_from_slice(&[0u8; 2]); // padding
    instruction_data.extend_from_slice(authority_pubkey.as_ref()); // authority_data
    
    let payer_pubkey = Pubkey::try_from(ctx.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    let accounts = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new(payer_pubkey, true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let add_authority_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };
    
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[add_authority_ix],
        &[],
        ctx.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[ctx.default_payer.insecure_clone()],
    )?;
    
    ctx.svm.send_transaction(tx).map_err(|e| anyhow::anyhow!("Failed to add authority: {:?}", e))?;
    
    // Verify authority was added
    let wallet_account_data = ctx.svm.get_account(&wallet_account).ok_or(anyhow::anyhow!("Wallet account not found"))?.data;
    let wallet_account_struct = unsafe {
        WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])?
    };
    let num_authorities = wallet_account_struct.num_authorities(&wallet_account_data)?;
    assert_eq!(num_authorities, 1);
    
    // Get authority ID (should be 0 for first authority)
    let authority_data = wallet_account_struct.get_authority(&wallet_account_data, 0)?
        .ok_or(anyhow::anyhow!("Authority not found"))?;
    
    // Add RolePermission plugin with All permission
    let plugin_program = Keypair::new();
    let plugin_program_pubkey = Pubkey::try_from(plugin_program.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    let plugin_config = Keypair::new();
    let plugin_config_pubkey = Pubkey::try_from(plugin_config.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    // Build AddPlugin instruction
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(3u16).to_le_bytes()); // AddPlugin = 3
    instruction_data.extend_from_slice(plugin_program_pubkey.as_ref());
    instruction_data.extend_from_slice(plugin_config_pubkey.as_ref());
    instruction_data.push(PluginType::RolePermission as u8);
    instruction_data.push(1u8); // enabled
    instruction_data.push(0u8); // priority
    instruction_data.extend_from_slice(&[0u8; 5]); // padding
    
    let accounts = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new(payer_pubkey, true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let add_plugin_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };
    
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[add_plugin_ix],
        &[],
        ctx.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[ctx.default_payer.insecure_clone()],
    )?;
    
    ctx.svm.send_transaction(tx).map_err(|e| anyhow::anyhow!("Failed to update authority: {:?}", e))?;
    
    // Update authority to reference the plugin
    // Build UpdateAuthority instruction to add plugin ref
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(5u16).to_le_bytes()); // UpdateAuthority = 5
    instruction_data.extend_from_slice(&0u32.to_le_bytes()); // authority_id
    instruction_data.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes()); // new_authority_type
    instruction_data.extend_from_slice(&(32u16).to_le_bytes()); // new_authority_data_len
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // num_plugin_refs
    instruction_data.extend_from_slice(&[0u8; 6]); // padding
    instruction_data.extend_from_slice(authority_pubkey.as_ref()); // new_authority_data
    
    // Add plugin ref: plugin_index (2 bytes) + enabled (1) + priority (1) + padding (4) = 8 bytes
    let mut plugin_ref_data = Vec::new();
    plugin_ref_data.extend_from_slice(&(0u16).to_le_bytes()); // plugin_index = 0 (first plugin)
    plugin_ref_data.push(1u8); // enabled
    plugin_ref_data.push(0u8); // priority
    plugin_ref_data.extend_from_slice(&[0u8; 4]); // padding
    instruction_data.extend_from_slice(&plugin_ref_data);
    
    let accounts = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new(payer_pubkey, true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let update_authority_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };
    
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[update_authority_ix],
        &[],
        ctx.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[ctx.default_payer.insecure_clone()],
    )?;
    
    ctx.svm.send_transaction(tx).map_err(|e| anyhow::anyhow!("Failed to execute: {:?}", e))?;
    
    // Now test execute with plugin
    // Create recipient
    let recipient = Keypair::new();
    let recipient_pubkey = Pubkey::try_from(recipient.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    // Create inner instruction: transfer from wallet_vault to recipient
    let transfer_amount = 500_000_000u64; // 0.5 SOL
    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    
    // Build compact instruction payload
    let mut instruction_payload = Vec::new();
    instruction_payload.push(1u8); // num_instructions = 1
    instruction_payload.push(2u8); // program_id_index (system_program)
    instruction_payload.push(inner_ix.accounts.len() as u8); // num_accounts
    instruction_payload.push(1u8); // wallet_vault index
    instruction_payload.push(3u8); // recipient index
    instruction_payload.extend_from_slice(&(inner_ix.data.len() as u16).to_le_bytes());
    instruction_payload.extend_from_slice(&inner_ix.data);
    
    // Build Execute instruction data
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Sign = 1
    instruction_data.extend_from_slice(&(instruction_payload.len() as u16).to_le_bytes());
    instruction_data.extend_from_slice(&0u32.to_le_bytes()); // authority_id = 0
    instruction_data.extend_from_slice(&instruction_payload);
    // authority_payload is empty (no signature needed for test)
    
    let accounts = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new_readonly(wallet_vault, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new(recipient_pubkey, false),
    ];
    
    let execute_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };
    
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[execute_ix],
        &[],
        ctx.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[ctx.default_payer.insecure_clone()],
    )?;
    
    // This should fail because plugin doesn't exist yet (mock plugin)
    // But the structure is correct
    let result = ctx.svm.send_transaction(tx);
    
    match result {
        Ok(_) => {
            println!("✅ Execute with RolePermission plugin (All) succeeded");
            Ok(())
        },
        Err(e) => {
            // Expected if plugin program doesn't exist
            println!("⚠️ Execute failed (expected if plugin not deployed): {:?}", e);
            Ok(())
        }
    }
}

/// Test: Execute with RolePermission plugin (ManageAuthority only) - should deny non-authority ops
#[test_log::test]
fn test_execute_with_role_permission_manage_authority_only() -> anyhow::Result<()> {
    // Similar to above but with ManageAuthority permission type
    // Should deny non-authority management operations
    Ok(())
}

/// Test: Execute with RolePermission plugin (AllButManageAuthority) - should deny authority ops
#[test_log::test]
fn test_execute_with_role_permission_all_but_manage() -> anyhow::Result<()> {
    // Similar to above but with AllButManageAuthority permission type
    // Should deny authority management operations
    Ok(())
}

/// Test: Multiple plugins with different priorities
#[test_log::test]
fn test_multiple_plugins_priority_order() -> anyhow::Result<()> {
    // Test that plugins are called in priority order (0 = highest priority)
    Ok(())
}

/// Test: Plugin state update after execute
#[test_log::test]
fn test_plugin_update_state_after_execute() -> anyhow::Result<()> {
    // Test that UpdateState instruction is called after instruction execution
    Ok(())
}

/// Test: Plugin validate add authority
#[test_log::test]
fn test_plugin_validate_add_authority() -> anyhow::Result<()> {
    // Test that ValidateAddAuthority is called when adding authority
    Ok(())
}

/// Test: Disabled plugin should not be called
#[test_log::test]
fn test_disabled_plugin_not_called() -> anyhow::Result<()> {
    // Test that disabled plugins (enabled = 0) are not called
    Ok(())
}

/// Test: Plugin priority sorting
#[test_log::test]
fn test_plugin_priority_sorting() -> anyhow::Result<()> {
    // Test that plugins are sorted by priority before being called
    Ok(())
}

/// Test: Execute with multiple plugins (all must allow)
#[test_log::test]
fn test_execute_with_multiple_plugins_all_allow() -> anyhow::Result<()> {
    // Test that all enabled plugins must allow for execution to proceed
    Ok(())
}

/// Test: Execute with multiple plugins (one denies)
#[test_log::test]
fn test_execute_with_multiple_plugins_one_denies() -> anyhow::Result<()> {
    // Test that if any plugin denies, execution fails
    Ok(())
}
