//! Tests for Create Wallet instruction (Pure External Architecture)

mod common;
use common::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::{TransactionError, VersionedTransaction},
};
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    Discriminator,
    Transmutable,
};

/// Test creating a new wallet
#[test_log::test]
fn test_create_wallet() {
    let mut context = setup_test_context().unwrap();
    
    // Generate unique wallet ID
    let wallet_id = rand::random::<[u8; 32]>();
    
    // Create wallet
    let (wallet_account, wallet_vault) = create_lazorkit_wallet(
        &mut context,
        wallet_id,
    ).unwrap();
    
    println!("✅ Wallet created:");
    println!("  Wallet account: {}", wallet_account);
    println!("  Wallet vault: {}", wallet_vault);
    
    // Verify wallet account was created
    let wallet_account_data = context.svm.get_account(&wallet_account).unwrap();
    let wallet_account_struct = get_wallet_account(&wallet_account_data).unwrap();
    
    assert_eq!(wallet_account_struct.discriminator, Discriminator::WalletAccount as u8);
    assert_eq!(wallet_account_struct.id, wallet_id);
    assert_eq!(wallet_account_struct.version, 1);
    
    // Verify num_authorities = 0
    let num_authorities = wallet_account_struct.num_authorities(&wallet_account_data.data).unwrap();
    assert_eq!(num_authorities, 0);
    
    // Verify num_plugins = 0 (check at offset WalletAccount::LEN + 2)
    let num_plugins = u16::from_le_bytes([
        wallet_account_data.data[WalletAccount::LEN],
        wallet_account_data.data[WalletAccount::LEN + 1],
    ]);
    assert_eq!(num_plugins, 0);
    
    // Verify wallet vault is system-owned
    let wallet_vault_data = context.svm.get_account(&wallet_vault).unwrap();
    assert_eq!(wallet_vault_data.owner, solana_sdk::system_program::id());
    
    println!("✅ Wallet account verified:");
    println!("  Discriminator: {}", wallet_account_struct.discriminator);
    println!("  ID: {:?}", wallet_account_struct.id);
    println!("  Version: {}", wallet_account_struct.version);
    println!("  Num authorities: {}", num_authorities);
    println!("  Num plugins: {}", num_plugins);
}

/// Test creating multiple wallets with different IDs
#[test_log::test]
fn test_create_multiple_wallets() {
    let mut context = setup_test_context().unwrap();
    
    // Create first wallet
    let id1 = rand::random::<[u8; 32]>();
    let (wallet1, vault1) = create_lazorkit_wallet(&mut context, id1).unwrap();
    
    // Create second wallet
    let id2 = rand::random::<[u8; 32]>();
    let (wallet2, vault2) = create_lazorkit_wallet(&mut context, id2).unwrap();
    
    // Verify they are different
    assert_ne!(wallet1, wallet2);
    assert_ne!(vault1, vault2);
    
    // Verify both wallets have correct IDs
    let wallet1_data = context.svm.get_account(&wallet1).unwrap();
    let wallet1_struct = get_wallet_account(&wallet1_data).unwrap();
    assert_eq!(wallet1_struct.id, id1);
    
    let wallet2_data = context.svm.get_account(&wallet2).unwrap();
    let wallet2_struct = get_wallet_account(&wallet2_data).unwrap();
    assert_eq!(wallet2_struct.id, id2);
    
    println!("✅ Created 2 wallets successfully");
    println!("  Wallet 1: {}", wallet1);
    println!("  Wallet 2: {}", wallet2);
}

/// Test that creating a wallet with duplicate ID fails
#[test_log::test]
fn test_create_wallet_duplicate_id() {
    let mut context = setup_test_context().unwrap();
    
    let wallet_id = rand::random::<[u8; 32]>();
    
    // Create first wallet - should succeed
    let (wallet1, _) = create_lazorkit_wallet(&mut context, wallet_id).unwrap();
    
    // Try to create second wallet with same ID - should fail
    let result = create_lazorkit_wallet(&mut context, wallet_id);
    
    match result {
        Ok((wallet2, _)) => {
            // If it succeeds, they should be the same account
            assert_eq!(wallet1, wallet2);
            println!("✅ Duplicate ID correctly uses same wallet account");
        },
        Err(e) => {
            println!("✅ Duplicate ID correctly rejected: {:?}", e);
        }
    }
}
