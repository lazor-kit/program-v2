//! Tests for Add Authority instruction (Pure External Architecture)

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
    position::Position,
    authority::AuthorityType,
    Discriminator,
    Transmutable,
};

/// Test adding Ed25519 authority to wallet
#[test_log::test]
fn test_add_authority_ed25519() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();
    
    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();
    
    // Create new authority keypair
    let new_authority = Keypair::new();
    let new_authority_pubkey = new_authority.pubkey();
    
    // Get Ed25519 authority data (just the pubkey bytes)
    let authority_data = new_authority_pubkey.to_bytes();
    
    // Build AddAuthority instruction
    // Format: [instruction: u16, new_authority_type: u16, new_authority_data_len: u16, 
    //          num_plugin_refs: u16, padding: [u8; 2], authority_data]
    // Note: instruction discriminator (2 bytes) is parsed separately in process_action
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2 (discriminator, parsed separately)
    // Args (after discriminator): new_authority_type, new_authority_data_len, num_plugin_refs, padding
    instruction_data.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes()); // Ed25519 = 1
    instruction_data.extend_from_slice(&(authority_data.len() as u16).to_le_bytes()); // authority_data_len
    instruction_data.extend_from_slice(&0u16.to_le_bytes()); // num_plugin_refs = 0
    instruction_data.extend_from_slice(&[0u8; 2]); // padding
    instruction_data.extend_from_slice(&authority_data);
    
    // Build accounts
    let payer_program_pubkey = context.default_payer.pubkey();
    let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
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
    
    // Build and send transaction
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[add_authority_ix],
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
            // Verify authority was added
            let wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
            let wallet_account_data = get_wallet_account(&wallet_account_info).unwrap();
            
            let num_authorities = wallet_account_data.num_authorities(&wallet_account_info.data).unwrap();
            assert_eq!(num_authorities, 1, "Should have 1 authority");
            
            // Get the authority
            let authority_data_result = wallet_account_data.get_authority(&wallet_account_info.data, 0)?;
            assert!(authority_data_result.is_some(), "Authority should exist");
            
            let authority_data = authority_data_result.unwrap();
            assert_eq!(authority_data.position.authority_type, AuthorityType::Ed25519 as u16);
            assert_eq!(authority_data.position.id, 0);
            assert_eq!(authority_data.authority_data, new_authority_pubkey.to_bytes().to_vec());
            
            println!("✅ Add authority Ed25519 succeeded");
            Ok(())
        },
        Err(e) => {
            println!("❌ Add authority failed: {:?}", e);
            Err(anyhow::anyhow!("Add authority failed: {:?}", e))
        }
    }
}

/// Test adding multiple authorities
#[test_log::test]
fn test_add_multiple_authorities() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();
    
    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();
    
    // Add first authority
    let authority1 = Keypair::new();
    let authority1_data = authority1.pubkey().to_bytes();
    
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(2u16).to_le_bytes());
    instruction_data.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes());
    instruction_data.extend_from_slice(&(authority1_data.len() as u16).to_le_bytes());
    instruction_data.extend_from_slice(&0u16.to_le_bytes());
    instruction_data.extend_from_slice(&[0u8; 2]);
    instruction_data.extend_from_slice(&authority1_data);
    
    let payer_program_pubkey = context.default_payer.pubkey();
    let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
        .expect("Failed to convert Pubkey");
    
    let accounts = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new(payer_pubkey, true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    
    let add_authority_ix1 = Instruction {
        program_id: lazorkit_program_id(),
        accounts: accounts.clone(),
        data: instruction_data,
    };
    
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[add_authority_ix1],
        &[],
        context.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone()],
    )?;
    
    context.svm.send_transaction(tx).map_err(|e| anyhow::anyhow!("Failed to add first authority: {:?}", e))?;
    
    // Add second authority
    let authority2 = Keypair::new();
    let authority2_data = authority2.pubkey().to_bytes();
    
    let mut instruction_data2 = Vec::new();
    instruction_data2.extend_from_slice(&(2u16).to_le_bytes()); // discriminator
    instruction_data2.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes());
    instruction_data2.extend_from_slice(&(authority2_data.len() as u16).to_le_bytes());
    instruction_data2.extend_from_slice(&0u16.to_le_bytes());
    instruction_data2.extend_from_slice(&[0u8; 2]);
    instruction_data2.extend_from_slice(&authority2_data);
    
    let add_authority_ix2 = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data2,
    };
    
    let message2 = v0::Message::try_compile(
        &payer_pubkey,
        &[add_authority_ix2],
        &[],
        context.svm.latest_blockhash(),
    )?;
    
    let tx2 = VersionedTransaction::try_new(
        VersionedMessage::V0(message2),
        &[context.default_payer.insecure_clone()],
    )?;
    
    context.svm.send_transaction(tx2).map_err(|e| anyhow::anyhow!("Failed to add second authority: {:?}", e))?;
    
    // Verify both authorities exist
    let wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
    let wallet_account_data = get_wallet_account(&wallet_account_info).unwrap();
    
    let num_authorities = wallet_account_data.num_authorities(&wallet_account_info.data).unwrap();
    assert_eq!(num_authorities, 2, "Should have 2 authorities");
    
    // Verify first authority (ID 0)
    let auth1 = wallet_account_data.get_authority(&wallet_account_info.data, 0)?.unwrap();
    assert_eq!(auth1.position.id, 0);
    assert_eq!(auth1.authority_data, authority1.pubkey().to_bytes().to_vec());
    
    // Verify second authority (ID 1)
    let auth2 = wallet_account_data.get_authority(&wallet_account_info.data, 1)?.unwrap();
    assert_eq!(auth2.position.id, 1);
    assert_eq!(auth2.authority_data, authority2.pubkey().to_bytes().to_vec());
    
    println!("✅ Add multiple authorities succeeded");
    Ok(())
}
