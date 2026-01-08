//! Tests for UpdatePlugin instruction (Pure External Architecture)

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

/// Test updating a plugin in wallet
#[test_log::test]
fn test_update_plugin() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();

    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();

    // Add plugin
    let plugin_program = Keypair::new();
    let plugin_config = Keypair::new();
    let mut add_instruction_data = Vec::new();
    add_instruction_data.extend_from_slice(&(3u16).to_le_bytes()); // AddPlugin = 3
    add_instruction_data.extend_from_slice(plugin_program.pubkey().as_ref());
    add_instruction_data.extend_from_slice(plugin_config.pubkey().as_ref());
    add_instruction_data.push(PluginType::RolePermission as u8);
    add_instruction_data.push(1u8); // enabled
    add_instruction_data.push(0u8); // priority
    add_instruction_data.extend_from_slice(&[0u8; 5]); // padding

    let payer_pubkey = context.default_payer.pubkey();
    let add_plugin_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(wallet_account, false),
            AccountMeta::new(payer_pubkey, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: add_instruction_data,
    };

    let add_message = v0::Message::try_compile(
        &payer_pubkey,
        &[add_plugin_ix],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile add message: {:?}", e))?;

    let add_tx = VersionedTransaction::try_new(
        VersionedMessage::V0(add_message),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create add transaction: {:?}", e))?;

    let add_result = context.svm.send_transaction(add_tx);
    if add_result.is_err() {
        return Err(anyhow::anyhow!("Failed to add plugin: {:?}", add_result.unwrap_err()));
    }
    println!("✅ Added plugin (index: 0)");

    // Update plugin: disable it and change priority
    // UpdatePluginArgs: plugin_index (2) + enabled (1) + priority (1) + padding (4) = 8 bytes (aligned)
    let mut update_instruction_data = Vec::new();
    update_instruction_data.extend_from_slice(&(5u16).to_le_bytes()); // UpdatePlugin = 5
    update_instruction_data.extend_from_slice(&0u16.to_le_bytes()); // plugin_index = 0
    update_instruction_data.push(0u8); // enabled = 0 (disable)
    update_instruction_data.push(5u8); // priority = 5
    update_instruction_data.extend_from_slice(&[0u8; 4]); // padding

    let update_plugin_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(wallet_account, false),
            AccountMeta::new(payer_pubkey, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: update_instruction_data,
    };

    let update_message = v0::Message::try_compile(
        &payer_pubkey,
        &[update_plugin_ix],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile update message: {:?}", e))?;

    let update_tx = VersionedTransaction::try_new(
        VersionedMessage::V0(update_message),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create update transaction: {:?}", e))?;

    let update_result = context.svm.send_transaction(update_tx);
    match update_result {
        Ok(_) => {
            println!("✅ Update plugin succeeded");
            
            // Verify wallet account state
            let updated_wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
            let updated_wallet_data = get_wallet_account(&updated_wallet_account_info).unwrap();
            
            let plugins = updated_wallet_data.get_plugins(&updated_wallet_account_info.data).unwrap();
            assert_eq!(plugins.len(), 1);
            assert_eq!(plugins[0].enabled, 0, "Plugin should be disabled");
            assert_eq!(plugins[0].priority, 5, "Plugin priority should be 5");
            
            Ok(())
        },
        Err(e) => {
            println!("❌ Update plugin failed: {:?}", e);
            Err(anyhow::anyhow!("Update plugin failed: {:?}", e))
        }
    }
}
