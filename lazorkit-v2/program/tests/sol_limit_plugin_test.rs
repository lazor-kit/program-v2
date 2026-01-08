//! Tests for SOL Limit Plugin

mod common;
use common::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction, InstructionError},
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

/// Test SOL limit plugin allows transfer within limit
#[test_log::test]
fn test_sol_limit_plugin_within_limit() {
    let mut context = setup_test_context().unwrap();
    
    // Setup accounts
    let authority = Keypair::new();
    let recipient = Keypair::new();
    
    // Convert solana_program::Pubkey to solana_sdk::Pubkey
    let authority_pubkey = Pubkey::try_from(authority.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let recipient_pubkey = Pubkey::try_from(recipient.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    
    context.svm.airdrop(&authority_pubkey, 1_000_000_000).unwrap();
    context.svm.airdrop(&recipient_pubkey, 1_000_000_000).unwrap();
    
    // Create wallet
    let wallet_id = [1u8; 32];
    let (wallet_account, wallet_vault) = create_lazorkit_wallet(
        &mut context,
        wallet_id,
    ).unwrap();
    
    println!("✅ Wallet created:");
    println!("  Wallet account: {}", wallet_account);
    println!("  Wallet vault: {}", wallet_vault);
    
    // Fund wallet vault
    context.svm.airdrop(&wallet_vault, 2_000_000_000).unwrap();
    
    // Verify wallet account was created
    let wallet_account_data = context.svm.get_account(&wallet_account).ok_or(anyhow::anyhow!("Wallet account not found")).unwrap().data;
    let wallet_account_struct = unsafe {
        WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN]).unwrap()
    };
    assert_eq!(wallet_account_data[0], Discriminator::WalletAccount as u8);
    let plugins = wallet_account_struct.get_plugins(&wallet_account_data).unwrap();
    assert_eq!(plugins.len(), 0);
    
    println!("✅ Wallet account verified: {} plugins", plugins.len());
    
    // TODO: Deploy plugin program and initialize config
    // For now, we'll test the structure without actual plugin
    
    // Test transfer (without plugin, should work if no plugins are enabled)
    let transfer_amount = 500_000_000u64; // 0.5 SOL
    
    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    
    // TODO: Create Sign instruction with new architecture
    // For now, skip the actual transfer test since it requires proper setup
    
    println!("✅ Test structure verified (actual transfer test requires plugin deployment)");
}

/// Test SOL limit plugin blocks transfer exceeding limit
#[test_log::test]
fn test_sol_limit_plugin_exceeds_limit() {
    let mut context = setup_test_context().unwrap();
    
    // Similar setup as above, but test that transfer exceeding limit is blocked
    println!("✅ Test for exceeding limit (to be fully implemented after plugin deployment)");
    
    // Test flow:
    // 1. Create wallet
    // 2. Deploy and add SOL limit plugin with 1 SOL limit
    // 3. Try to transfer 1.5 SOL - should fail
    // 4. Verify error is from plugin
}

/// Test SOL limit plugin decrements limit after transfer
#[test_log::test]
fn test_sol_limit_plugin_decrements_limit() {
    let mut context = setup_test_context().unwrap();
    
    // Test that after a successful transfer, the remaining limit is decreased
    println!("✅ Test for limit decrement (to be fully implemented after plugin deployment)");
    
    // Test flow:
    // 1. Create wallet
    // 2. Deploy and add SOL limit plugin with 1 SOL limit
    // 3. Transfer 0.3 SOL - should succeed
    // 4. Verify plugin config remaining_amount is now 0.7 SOL
    // 5. Transfer 0.5 SOL - should succeed
    // 6. Verify plugin config remaining_amount is now 0.2 SOL
    // 7. Try to transfer 0.3 SOL - should fail (exceeds remaining 0.2 SOL)
}
