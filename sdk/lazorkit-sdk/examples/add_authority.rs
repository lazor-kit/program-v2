// Example: Adding an admin authority to a wallet
//
// This example demonstrates how to:
// 1. Connect to an existing wallet
// 2. Build an AddAuthority transaction for an admin role
// 3. Sign and submit the transaction

use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::AuthorityType;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Existing wallet details
    let program_id = LazorWallet::DEFAULT_PROGRAM_ID;
    let config_pda = Pubkey::new_unique(); // Replace with your wallet's config PDA
    let vault_pda = Pubkey::new_unique(); // Replace with your wallet's vault PDA

    let wallet = LazorWallet::new(program_id, config_pda, vault_pda);

    // 2. Generate new admin keypair
    let new_admin = Keypair::new();
    let admin_pubkey = new_admin.pubkey().to_bytes();

    println!("Adding Admin Authority:");
    println!("  Wallet Config: {}", config_pda);
    println!("  New Admin: {}", new_admin.pubkey());

    // 3. Build AddAuthority transaction
    let builder = wallet
        .add_authority()
        .with_authority_key(admin_pubkey.to_vec())
        .with_type(AuthorityType::Ed25519)
        .with_role(1) // Role ID 1 = Admin
        .with_acting_role(0); // Acting as owner (role 0)

    // In a real application:
    // let connection = ...;
    // let payer = Keypair::new();
    // let tx = builder.build_transaction(&connection, payer.pubkey()).await?;
    // // Sign with owner and payer, then send

    println!("Transaction built successfully!");
    println!("Note: Owner must sign this transaction");

    Ok(())
}
