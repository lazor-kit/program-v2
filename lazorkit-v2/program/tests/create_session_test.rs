//! Tests for CreateSession instruction (Pure External Architecture)

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

/// Test creating a session for an authority
#[test_log::test]
fn test_create_session() -> anyhow::Result<()> {
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

    // Create session for authority
    // CreateSessionArgs: authority_id (4) + session_duration (8) + session_key (32) = 44 bytes, but aligned to 8 = 48 bytes
    let session_key = rand::random::<[u8; 32]>();
    let session_duration = 1000u64; // 1000 slots
    
    let mut create_session_instruction_data = Vec::new();
    create_session_instruction_data.extend_from_slice(&(8u16).to_le_bytes()); // CreateSession = 8
    create_session_instruction_data.extend_from_slice(&0u32.to_le_bytes()); // authority_id = 0
    create_session_instruction_data.extend_from_slice(&session_duration.to_le_bytes()); // session_duration
    create_session_instruction_data.extend_from_slice(&session_key); // session_key (32 bytes)
    create_session_instruction_data.extend_from_slice(&[0u8; 4]); // additional padding to align to 8 bytes (total 48 bytes)

    let create_session_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(wallet_account, false),
            AccountMeta::new(payer_pubkey, true), // payer for rent if needed
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: create_session_instruction_data,
    };

    let create_session_message = v0::Message::try_compile(
        &payer_pubkey,
        &[create_session_ix],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile create session message: {:?}", e))?;

    let create_session_tx = VersionedTransaction::try_new(
        VersionedMessage::V0(create_session_message),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create create session transaction: {:?}", e))?;

    let create_session_result = context.svm.send_transaction(create_session_tx);
    match create_session_result {
        Ok(_) => {
            println!("✅ Create session succeeded");
            
            // Verify wallet account state
            let updated_wallet_account_info = context.svm.get_account(&wallet_account).unwrap();
            let updated_wallet_data = get_wallet_account(&updated_wallet_account_info).unwrap();
            
            // Verify authority was converted to session-based
            let session_authority = updated_wallet_data.get_authority(&updated_wallet_account_info.data, 0)?;
            assert!(session_authority.is_some(), "Authority ID 0 should still exist");
            let auth_data = session_authority.unwrap();
            
            // Verify authority type changed to Ed25519Session (2)
            assert_eq!(auth_data.position.authority_type, 2, "Authority type should be Ed25519Session (2)");
            
            // Verify authority data length increased (original 32 + session data 48 = 80 bytes)
            assert_eq!(auth_data.position.authority_length, 80, "Authority data length should be 80 bytes (32 + 48)");
            
            Ok(())
        },
        Err(e) => {
            println!("❌ Create session failed: {:?}", e);
            Err(anyhow::anyhow!("Create session failed: {:?}", e))
        }
    }
}
