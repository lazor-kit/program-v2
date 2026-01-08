//! Tests for UpdateAuthority instruction (Pure External Architecture)

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

/// Test updating an authority in wallet
#[test_log::test]
fn test_update_authority() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();

    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();

    // Add authority
    let authority = Keypair::new();
    let authority_data = authority.pubkey().to_bytes();
    let mut add_instruction_data = Vec::new();
    add_instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    add_instruction_data.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes());
    add_instruction_data.extend_from_slice(&(authority_data.len() as u16).to_le_bytes());
    add_instruction_data.extend_from_slice(&0u16.to_le_bytes()); // num_plugin_refs
    add_instruction_data.extend_from_slice(&[0u8; 2]); // padding
    add_instruction_data.extend_from_slice(&authority_data);

    let payer_pubkey = context.default_payer.pubkey();
    let add_authority_ix = Instruction {
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
        &[add_authority_ix],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile add message: {:?}", e))?;

    let add_tx = VersionedTransaction::try_new(
        VersionedMessage::V0(add_message),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create add transaction: {:?}", e))?;

    let add_result = context.svm.send_transaction(add_tx);
    if add_result.is_err() {
        return Err(anyhow::anyhow!("Failed to add authority: {:?}", add_result.unwrap_err()));
    }
    println!("✅ Added authority (ID: 0)");

    // Update authority: change to a new pubkey (same type, different data)
    // UpdateAuthorityArgs: authority_id (4) + new_authority_type (2) + new_authority_data_len (2) + num_plugin_refs (2) + padding (2) = 12 bytes, but aligned to 8 = 16 bytes
    let new_authority = Keypair::new();
    let new_authority_data = new_authority.pubkey().to_bytes();
    
    let mut update_instruction_data = Vec::new();
    update_instruction_data.extend_from_slice(&(6u16).to_le_bytes()); // UpdateAuthority = 6
    update_instruction_data.extend_from_slice(&0u32.to_le_bytes()); // authority_id = 0
    update_instruction_data.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes()); // new_authority_type
    update_instruction_data.extend_from_slice(&(new_authority_data.len() as u16).to_le_bytes()); // new_authority_data_len
    update_instruction_data.extend_from_slice(&0u16.to_le_bytes()); // num_plugin_refs
    update_instruction_data.extend_from_slice(&[0u8; 2]); // padding (2 bytes)
    update_instruction_data.extend_from_slice(&[0u8; 4]); // additional padding to align to 8 bytes (total 16 bytes)
    update_instruction_data.extend_from_slice(&new_authority_data); // new authority data

    let update_authority_ix = Instruction {
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
        &[update_authority_ix],
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
            println!("✅ Update authority succeeded");
            
            // Verify wallet account state
            let updated_wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
            let updated_wallet_data = get_wallet_account(&updated_wallet_account_info).unwrap();
            
            // Verify authority was updated
            let updated_authority = updated_wallet_data.get_authority(&updated_wallet_account_info.data, 0)?;
            assert!(updated_authority.is_some(), "Authority ID 0 should still exist");
            let auth_data = updated_authority.unwrap();
            assert_eq!(auth_data.authority_data, new_authority_data, "Authority data should be updated");
            
            Ok(())
        },
        Err(e) => {
            println!("❌ Update authority failed: {:?}", e);
            Err(anyhow::anyhow!("Update authority failed: {:?}", e))
        }
    }
}
