//! Tests for Add Plugin instruction (Pure External Architecture)

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
    Discriminator,
    Transmutable,
};

/// Test adding a plugin to wallet
#[test_log::test]
fn test_add_plugin() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();
    
    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();
    
    // Create mock plugin program and config
    let plugin_program = Keypair::new();
    let plugin_program_pubkey = Pubkey::try_from(plugin_program.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    let plugin_config = Keypair::new();
    let plugin_config_pubkey = Pubkey::try_from(plugin_config.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    // Build AddPlugin instruction
    // Format: [instruction: u16, program_id (32), config_account (32), plugin_type (1), enabled (1), priority (1), padding (5)]
    // Note: instruction discriminator (2 bytes) is parsed separately in process_action
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(3u16).to_le_bytes()); // AddPlugin = 3 (discriminator)
    // Args (after discriminator): program_id, config_account, plugin_type, enabled, priority, padding
    instruction_data.extend_from_slice(plugin_program_pubkey.as_ref()); // program_id (32 bytes)
    instruction_data.extend_from_slice(plugin_config_pubkey.as_ref()); // config_account (32 bytes)
    instruction_data.push(PluginType::RolePermission as u8); // plugin_type
    instruction_data.push(1u8); // enabled
    instruction_data.push(0u8); // priority
    instruction_data.extend_from_slice(&[0u8; 5]); // padding
    
    // Build accounts
    let payer_program_pubkey = context.default_payer.pubkey();
    let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
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
    
    // Build and send transaction
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[add_plugin_ix],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile message: {:?}", e))?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create transaction: {:?}", e))?;
    
    let result = context.svm.send_transaction(tx);
    
    match result {
        Ok(_) => {
            // Verify plugin was added
            let wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
            let wallet_account_data = get_wallet_account(&wallet_account_info).unwrap();
            
            let plugins = wallet_account_data.get_plugins(&wallet_account_info.data).unwrap();
            assert_eq!(plugins.len(), 1, "Should have 1 plugin");
            
            let plugin = &plugins[0];
            assert_eq!(plugin.program_id.as_ref(), plugin_program_pubkey.as_ref());
            assert_eq!(plugin.config_account.as_ref(), plugin_config_pubkey.as_ref());
            assert_eq!(plugin.plugin_type(), PluginType::RolePermission);
            assert_eq!(plugin.enabled, 1);
            assert_eq!(plugin.priority, 0);
            
            println!("✅ Add plugin succeeded");
            Ok(())
        },
        Err(e) => {
            println!("❌ Add plugin failed: {:?}", e);
            Err(anyhow::anyhow!("Add plugin failed: {:?}", e))
        }
    }
}

/// Test adding multiple plugins
#[test_log::test]
fn test_add_multiple_plugins() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();
    
    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();
    
    // Add first plugin
    let plugin1_program = Keypair::new();
    let plugin1_program_pubkey = Pubkey::try_from(plugin1_program.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let plugin1_config = Keypair::new();
    let plugin1_config_pubkey = Pubkey::try_from(plugin1_config.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    let mut instruction_data1 = Vec::new();
    instruction_data1.extend_from_slice(&(3u16).to_le_bytes());
    instruction_data1.extend_from_slice(plugin1_program_pubkey.as_ref());
    instruction_data1.extend_from_slice(plugin1_config_pubkey.as_ref());
    instruction_data1.push(PluginType::RolePermission as u8);
    instruction_data1.push(1u8);
    instruction_data1.push(0u8);
    instruction_data1.extend_from_slice(&[0u8; 5]);
    
    let payer_program_pubkey = context.default_payer.pubkey();
    let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
        .expect("Failed to convert Pubkey");
    
    let accounts1 = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new(payer_pubkey, true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let add_plugin_ix1 = Instruction {
        program_id: lazorkit_program_id(),
        accounts: accounts1.clone(),
        data: instruction_data1,
    };
    
    let message1 = v0::Message::try_compile(
        &payer_pubkey,
        &[add_plugin_ix1],
        &[],
        context.svm.latest_blockhash(),
    )?;
    
    let tx1 = VersionedTransaction::try_new(
        VersionedMessage::V0(message1),
        &[context.default_payer.insecure_clone()],
    )?;
    
    context.svm.send_transaction(tx1).map_err(|e| anyhow::anyhow!("Failed to add first plugin: {:?}", e))?;
    
    // Add second plugin
    let plugin2_program = Keypair::new();
    let plugin2_program_pubkey = Pubkey::try_from(plugin2_program.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let plugin2_config = Keypair::new();
    let plugin2_config_pubkey = Pubkey::try_from(plugin2_config.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    let mut instruction_data2 = Vec::new();
    instruction_data2.extend_from_slice(&(3u16).to_le_bytes());
    instruction_data2.extend_from_slice(plugin2_program_pubkey.as_ref());
    instruction_data2.extend_from_slice(plugin2_config_pubkey.as_ref());
    instruction_data2.push(PluginType::SolLimit as u8);
    instruction_data2.push(1u8);
    instruction_data2.push(1u8); // priority 1
    instruction_data2.extend_from_slice(&[0u8; 5]);
    
    let add_plugin_ix2 = Instruction {
        program_id: lazorkit_program_id(),
        accounts: accounts1,
        data: instruction_data2,
    };
    
    let message2 = v0::Message::try_compile(
        &payer_pubkey,
        &[add_plugin_ix2],
        &[],
        context.svm.latest_blockhash(),
    )?;
    
    let tx2 = VersionedTransaction::try_new(
        VersionedMessage::V0(message2),
        &[context.default_payer.insecure_clone()],
    )?;
    
    context.svm.send_transaction(tx2).map_err(|e| anyhow::anyhow!("Failed to add second plugin: {:?}", e))?;
    
    // Verify both plugins exist
    let wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
    let wallet_account_data = get_wallet_account(&wallet_account_info).unwrap();
    
    let plugins = wallet_account_data.get_plugins(&wallet_account_info.data).unwrap();
    assert_eq!(plugins.len(), 2, "Should have 2 plugins");
    
    // Verify first plugin
    assert_eq!(plugins[0].program_id.as_ref(), plugin1_program_pubkey.as_ref());
    assert_eq!(plugins[0].plugin_type(), PluginType::RolePermission);
    assert_eq!(plugins[0].priority, 0);
    
    // Verify second plugin
    assert_eq!(plugins[1].program_id.as_ref(), plugin2_program_pubkey.as_ref());
    assert_eq!(plugins[1].plugin_type(), PluginType::SolLimit);
    assert_eq!(plugins[1].priority, 1);
    
    println!("✅ Add multiple plugins succeeded");
    Ok(())
}
