//! Tests for RemoveAuthority instruction (Pure External Architecture)

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
    authority::AuthorityType,
    Transmutable,
};

/// Test removing an authority from wallet
#[test_log::test]
fn test_remove_authority() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();

    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();

    // Add first authority
    let authority_1 = Keypair::new();
    let authority_data_1 = authority_1.pubkey().to_bytes();
    let mut instruction_data_1 = Vec::new();
    instruction_data_1.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    instruction_data_1.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes());
    instruction_data_1.extend_from_slice(&(authority_data_1.len() as u16).to_le_bytes());
    instruction_data_1.extend_from_slice(&0u16.to_le_bytes()); // num_plugin_refs
    instruction_data_1.extend_from_slice(&[0u8; 2]); // padding
    instruction_data_1.extend_from_slice(&authority_data_1);

    let payer_pubkey = context.default_payer.pubkey();
    let add_authority_ix_1 = Instruction {
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
        &[add_authority_ix_1],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile message 1: {:?}", e))?;

    let tx_1 = VersionedTransaction::try_new(
        VersionedMessage::V0(message_1),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create transaction 1: {:?}", e))?;

    let result_1 = context.svm.send_transaction(tx_1);
    if result_1.is_err() {
        return Err(anyhow::anyhow!("Failed to add first authority: {:?}", result_1.unwrap_err()));
    }
    println!("✅ Added first authority (ID: 0)");

    // Add second authority
    let authority_2 = Keypair::new();
    let authority_data_2 = authority_2.pubkey().to_bytes();
    let mut instruction_data_2 = Vec::new();
    instruction_data_2.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    instruction_data_2.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes());
    instruction_data_2.extend_from_slice(&(authority_data_2.len() as u16).to_le_bytes());
    instruction_data_2.extend_from_slice(&0u16.to_le_bytes()); // num_plugin_refs
    instruction_data_2.extend_from_slice(&[0u8; 2]); // padding
    instruction_data_2.extend_from_slice(&authority_data_2);

    let add_authority_ix_2 = Instruction {
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
        &[add_authority_ix_2],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile message 2: {:?}", e))?;

    let tx_2 = VersionedTransaction::try_new(
        VersionedMessage::V0(message_2),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create transaction 2: {:?}", e))?;

    let result_2 = context.svm.send_transaction(tx_2);
    if result_2.is_err() {
        return Err(anyhow::anyhow!("Failed to add second authority: {:?}", result_2.unwrap_err()));
    }
    println!("✅ Added second authority (ID: 1)");

    // Verify we have 2 authorities
    let wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
    let wallet_data = get_wallet_account(&wallet_account_info).unwrap();
    assert_eq!(wallet_data.num_authorities(&wallet_account_info.data).unwrap(), 2);

    // Remove first authority (ID: 0)
    let mut remove_instruction_data = Vec::new();
    remove_instruction_data.extend_from_slice(&(7u16).to_le_bytes()); // RemoveAuthority = 7
    remove_instruction_data.extend_from_slice(&0u32.to_le_bytes()); // authority_id = 0
    remove_instruction_data.extend_from_slice(&[0u8; 4]); // padding

    let remove_authority_ix = Instruction {
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
        &[remove_authority_ix],
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
            println!("✅ Remove authority succeeded");
            
            // Verify wallet account state
            let updated_wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
            let updated_wallet_data = get_wallet_account(&updated_wallet_account_info).unwrap();
            assert_eq!(updated_wallet_data.num_authorities(&updated_wallet_account_info.data).unwrap(), 1);
            
            // Verify authority ID 0 is removed and ID 1 still exists
            let authority_0 = updated_wallet_data.get_authority(&updated_wallet_account_info.data, 0)?;
            assert!(authority_0.is_none(), "Authority ID 0 should be removed");
            
            let authority_1 = updated_wallet_data.get_authority(&updated_wallet_account_info.data, 1)?;
            assert!(authority_1.is_some(), "Authority ID 1 should still exist");
            
            Ok(())
        },
        Err(e) => {
            println!("❌ Remove authority failed: {:?}", e);
            Err(anyhow::anyhow!("Remove authority failed: {:?}", e))
        }
    }
}
