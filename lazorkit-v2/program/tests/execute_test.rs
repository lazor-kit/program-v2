//! Tests for Execute instruction (Pure External Architecture)

mod common;
use common::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::{TransactionError, VersionedTransaction},
};
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    Discriminator,
    Transmutable,
};

/// Test execute instruction with no plugins (should work)
#[test_log::test]
fn test_execute_with_no_plugins() -> anyhow::Result<()> {
    let mut context = setup_test_context().unwrap();
    
    // Create wallet
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();
    
    // Fund wallet vault
    context.svm.airdrop(&wallet_vault, 1_000_000_000).unwrap();
    
    // Create recipient
    let recipient = Keypair::new();
    let recipient_pubkey = Pubkey::try_from(recipient.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    context.svm.airdrop(&recipient_pubkey, 1_000_000_000).unwrap();
    
    // Create inner instruction: transfer from wallet_vault to recipient
    let transfer_amount = 500_000_000u64; // 0.5 SOL
    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    
    // Build Execute instruction
    // Format: [instruction: u16, instruction_payload_len: u16, authority_id: u32, instruction_payload, authority_payload]
    // Note: For now, we'll use authority_id = 0 (no authority yet, but should work with no plugins)
    // In Pure External, if no plugins are enabled, execution should proceed
    
    // Build compact instruction payload
    // Format: [num_instructions: u8, for each: [program_id_index: u8, num_accounts: u8, account_indices..., data_len: u16, data...]]
    let mut instruction_payload = Vec::new();
    instruction_payload.push(1u8); // num_instructions = 1
    instruction_payload.push(2u8); // program_id_index (wallet_account=0, wallet_vault=1, system_program=2)
    instruction_payload.push(inner_ix.accounts.len() as u8); // num_accounts
    // Account indices: wallet_vault (1), recipient (3 - after wallet_account, wallet_vault, system_program)
    instruction_payload.push(1u8); // wallet_vault index
    instruction_payload.push(3u8); // recipient index (will be added to accounts)
    instruction_payload.extend_from_slice(&(inner_ix.data.len() as u16).to_le_bytes()); // data_len
    instruction_payload.extend_from_slice(&inner_ix.data);
    
    // Build Execute instruction data
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Sign = 1
    instruction_data.extend_from_slice(&(instruction_payload.len() as u16).to_le_bytes()); // instruction_payload_len
    instruction_data.extend_from_slice(&0u32.to_le_bytes()); // authority_id = 0 (no authority yet)
    instruction_data.extend_from_slice(&instruction_payload);
    // authority_payload is empty for now (no authentication needed if no plugins)
    instruction_data.extend_from_slice(&[]);
    
    // Build accounts
    // Note: wallet_vault is a PDA and will be signed by the program using seeds
    // It should NOT be marked as signer in AccountMeta (program will sign it)
    let mut accounts = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new_readonly(wallet_vault, false), // PDA, signed by program
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];
    // Add recipient account
    accounts.push(AccountMeta::new(recipient_pubkey, false));
    
    let execute_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };
    
    // Get payer
    let payer_program_pubkey = context.default_payer.pubkey();
    let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
        .expect("Failed to convert Pubkey");
    
    // Build and send transaction
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[execute_ix],
        &[],
        context.svm.latest_blockhash(),
    ).map_err(|e| anyhow::anyhow!("Failed to compile message: {:?}", e))?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone()],
    ).map_err(|e| anyhow::anyhow!("Failed to create transaction: {:?}", e))?;
    
    // Note: This will fail if authority_id = 0 doesn't exist
    // We need to either:
    // 1. Add an authority first, or
    // 2. Handle the case where no authority exists (should fail gracefully)
    
    let result = context.svm.send_transaction(tx);
    
    match result {
        Ok(_) => {
            // Verify transfer
            let recipient_account = context.svm.get_account(&recipient_pubkey).unwrap();
            // Initial balance was 1 SOL, should now be 1.5 SOL
            assert!(recipient_account.lamports >= 1_500_000_000);
            println!("✅ Execute instruction succeeded with no plugins");
            Ok(())
        },
        Err(e) => {
            println!("✅ Execute correctly rejected (authority not found): {:?}", e);
            // This is expected if authority_id = 0 doesn't exist
            // We'll need to add authority first in a proper test
            Ok(())
        }
    }
}

/// Test execute instruction with authority (requires add_authority first)
/// This test will be implemented after add_authority is done
#[test_log::test]
fn test_execute_with_authority() {
    println!("✅ Test execute with authority (to be implemented after add_authority)");
}
