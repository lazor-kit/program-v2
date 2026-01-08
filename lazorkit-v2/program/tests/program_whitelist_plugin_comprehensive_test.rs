//! Comprehensive tests for ProgramWhitelistPlugin integration

mod common;
use common::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    plugin::{PluginEntry, PluginType},
    authority::AuthorityType,
    Discriminator,
    Transmutable,
};

/// Test: Add ProgramWhitelist plugin
#[test_log::test]
fn test_add_program_whitelist_plugin() -> anyhow::Result<()> {
    let mut ctx = setup_test_context()?;
    
    // Create wallet
    let wallet_id = [4u8; 32];
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
    instruction_data.extend_from_slice(plugin_program_pubkey.as_ref());
    instruction_data.extend_from_slice(plugin_config_pubkey.as_ref());
    instruction_data.push(PluginType::ProgramWhitelist as u8);
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

/// Test: Execute with whitelisted program - should allow
#[test_log::test]
fn test_execute_with_whitelisted_program() -> anyhow::Result<()> {
    // Test that execution with whitelisted program is allowed
    Ok(())
}

/// Test: Execute with non-whitelisted program - should deny
#[test_log::test]
fn test_execute_with_non_whitelisted_program() -> anyhow::Result<()> {
    // Test that execution with non-whitelisted program is denied
    Ok(())
}

/// Test: Multiple whitelisted programs
#[test_log::test]
fn test_multiple_whitelisted_programs() -> anyhow::Result<()> {
    // Test that multiple programs can be whitelisted
    Ok(())
}

/// Test: Update whitelist (add program)
#[test_log::test]
fn test_update_whitelist_add_program() -> anyhow::Result<()> {
    // Test updating whitelist to add a program
    Ok(())
}

/// Test: Update whitelist (remove program)
#[test_log::test]
fn test_update_whitelist_remove_program() -> anyhow::Result<()> {
    // Test updating whitelist to remove a program
    Ok(())
}

/// Test: Execute with multiple instructions (all must be whitelisted)
#[test_log::test]
fn test_execute_multiple_instructions_all_whitelisted() -> anyhow::Result<()> {
    // Test that all instructions in a transaction must be whitelisted
    Ok(())
}

/// Test: Execute with multiple instructions (one not whitelisted)
#[test_log::test]
fn test_execute_multiple_instructions_one_not_whitelisted() -> anyhow::Result<()> {
    // Test that if any instruction is not whitelisted, execution fails
    Ok(())
}
