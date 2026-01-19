// Example: Creating a LazorKit wallet with Ed25519 authority
//
// This example demonstrates how to:
// 1. Generate a random wallet ID
// 2. Create a wallet with Ed25519 owner
// 3. Derive the config and vault PDAs

use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::AuthorityType;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Generate wallet ID (random 32 bytes)
    let wallet_id = rand::random::<[u8; 32]>();

    // 2. Generate owner keypair
    let owner = Keypair::new();
    let owner_pubkey = owner.pubkey().to_bytes();

    // 3. Build create wallet transaction
    let builder = LazorWallet::create()
        .with_payer(Pubkey::new_unique()) // Replace with actual payer
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Ed25519)
        .with_owner_authority_key(owner_pubkey.to_vec());

    // 4. Get the PDAs that will be created
    let (config_pda, vault_pda) = builder.get_pdas();

    println!("Creating LazorKit Wallet:");
    println!("  Wallet ID: {}", hex::encode(wallet_id));
    println!("  Config PDA: {}", config_pda);
    println!("  Vault PDA: {}", vault_pda);
    println!("  Owner: {}", owner.pubkey());

    // In a real application, you would:
    // let connection = ...;  // Your Solana connection
    // let tx = builder.build_transaction(&connection).await?;
    // // Sign and send transaction

    Ok(())
}
