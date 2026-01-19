// Example: Creating a session key for temporary access
//
// This example demonstrates how to:
// 1. Create a session key for a role
// 2. Set expiration duration
// 3. Use the session to sign transactions

use lazorkit_sdk::basic::wallet::LazorWallet;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Existing wallet
    let wallet = LazorWallet::new(
        LazorWallet::DEFAULT_PROGRAM_ID,
        Pubkey::new_unique(), // config_pda
        Pubkey::new_unique(), // vault_pda
    );

    // 2. Generate session key
    let session_keypair = Keypair::new();
    let session_key = session_keypair.pubkey().to_bytes();

    // 3. Session duration (in slots, ~400ms per slot)
    let duration_slots = 7200; // ~48 minutes

    println!("Creating Session Key:");
    println!("  Session Key: {}", session_keypair.pubkey());
    println!(
        "  Duration: {} slots (~{} minutes)",
        duration_slots,
        duration_slots * 400 / 1000 / 60
    );

    // 4. Build CreateSession transaction
    let builder = wallet
        .create_session()
        .with_role(0) // Create session for owner role
        .with_session_key(session_key)
        .with_duration(duration_slots);

    // In a real application:
    // let connection = ...;
    // let payer = Keypair::new();
    // let tx = builder.build_transaction(&connection, payer.pubkey()).await?;
    // // Sign with master key and payer, then send

    println!("Session transaction built!");
    println!("After creation, you can use the session key to sign transactions");
    println!("until it expires (slot: current + {})", duration_slots);

    Ok(())
}
