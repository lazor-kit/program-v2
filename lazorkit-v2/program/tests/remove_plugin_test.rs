//! Tests for RemovePlugin instruction (Pure External Architecture)

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
    Transmutable,
};

/// Test removing a plugin from wallet
#[test_log::test]
fn test_remove_plugin() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();

    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();

    // Add first plugin
    let plugin_program_1 = Keypair::new();
    let plugin_config_1 = Keypair::new();
    let mut instruction_data_1 = Vec::new();
    instruction_data_1.extend_from_slice(&(3u16).to_le_bytes()); // AddPlugin = 3
    instruction_data_1.extend_from_slice(plugin_program_1.pubkey().as_ref());
    instruction_data_1.extend_from_slice(plugin_config_1.pubkey().as_ref());
    instruction_data_1.push(PluginType::RolePermission as u8);
    instruction_data_1.push(1u8); // enabled
    instruction_data_1.push(0u8); // priority
    instruction_data_1.extend_from_slice(&[0u8; 5]); // padding

    let payer_pubkey = context.default_payer.pubkey();
    let add_plugin_ix_1 = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(wallet_account, false),
            AccountMeta::new(payer_pubkey, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: instruction_data_1,
    };

    let message_1 = v0::Message::try_compile(
        &payer_pubkey,
        &[add_plugin_ix_1],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile message 1: {:?}", e))?;

    let tx_1 = VersionedTransaction::try_new(
        VersionedMessage::V0(message_1),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create transaction 1: {:?}", e))?;

    let result_1 = context.svm.send_transaction(tx_1);
    if result_1.is_err() {
        return Err(anyhow::anyhow!("Failed to add first plugin: {:?}", result_1.unwrap_err()));
    }
    println!("✅ Added first plugin (index: 0)");

    // Add second plugin
    let plugin_program_2 = Keypair::new();
    let plugin_config_2 = Keypair::new();
    let mut instruction_data_2 = Vec::new();
    instruction_data_2.extend_from_slice(&(3u16).to_le_bytes()); // AddPlugin = 3
    instruction_data_2.extend_from_slice(plugin_program_2.pubkey().as_ref());
    instruction_data_2.extend_from_slice(plugin_config_2.pubkey().as_ref());
    instruction_data_2.push(PluginType::RolePermission as u8);
    instruction_data_2.push(1u8); // enabled
    instruction_data_2.push(1u8); // priority
    instruction_data_2.extend_from_slice(&[0u8; 5]); // padding

    let add_plugin_ix_2 = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(wallet_account, false),
            AccountMeta::new(payer_pubkey, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: instruction_data_2,
    };

    let message_2 = v0::Message::try_compile(
        &payer_pubkey,
        &[add_plugin_ix_2],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile message 2: {:?}", e))?;

    let tx_2 = VersionedTransaction::try_new(
        VersionedMessage::V0(message_2),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create transaction 2: {:?}", e))?;

    let result_2 = context.svm.send_transaction(tx_2);
    if result_2.is_err() {
        return Err(anyhow::anyhow!("Failed to add second plugin: {:?}", result_2.unwrap_err()));
    }
    println!("✅ Added second plugin (index: 1)");

    // Verify we have 2 plugins
    let wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
    let wallet_data = get_wallet_account(&wallet_account_info).unwrap();
    let plugins = wallet_data.get_plugins(&wallet_account_info.data).unwrap();
    assert_eq!(plugins.len(), 2);

    // Remove first plugin (index: 0)
    // RemovePluginArgs: plugin_index (2) + padding (2) = 4 bytes, but aligned to 8 bytes
    let mut remove_instruction_data = Vec::new();
    remove_instruction_data.extend_from_slice(&(4u16).to_le_bytes()); // RemovePlugin = 4 (check instruction.rs)
    remove_instruction_data.extend_from_slice(&0u16.to_le_bytes()); // plugin_index = 0
    remove_instruction_data.extend_from_slice(&[0u8; 2]); // padding (2 bytes)
    remove_instruction_data.extend_from_slice(&[0u8; 4]); // additional padding to align to 8 bytes

    let remove_plugin_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(wallet_account, false),
            AccountMeta::new(payer_pubkey, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: remove_instruction_data,
    };

    let remove_message = v0::Message::try_compile(
        &payer_pubkey,
        &[remove_plugin_ix],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile remove message: {:?}", e))?;

    let remove_tx = VersionedTransaction::try_new(
        VersionedMessage::V0(remove_message),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create remove transaction: {:?}", e))?;

    let remove_result = context.svm.send_transaction(remove_tx);
    match remove_result {
        Ok(_) => {
            println!("✅ Remove plugin succeeded");
            
            // Verify wallet account state
            let updated_wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
            let updated_wallet_data = get_wallet_account(&updated_wallet_account_info).unwrap();
            
            // Verify plugin at index 0 is now the second plugin (plugin_program_2)
            let plugins = updated_wallet_data.get_plugins(&updated_wallet_account_info.data).unwrap();
            assert_eq!(plugins.len(), 1);
            assert_eq!(plugins[0].program_id.as_ref(), plugin_program_2.pubkey().as_ref());
            
            Ok(())
        },
        Err(e) => {
            println!("❌ Remove plugin failed: {:?}", e);
            Err(anyhow::anyhow!("Remove plugin failed: {:?}", e))
        }
    }
}
