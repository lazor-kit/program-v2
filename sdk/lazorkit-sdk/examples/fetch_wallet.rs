// Example: Fetching wallet information and listing roles
//
// This example demonstrates how to:
// 1. Fetch a wallet by its config PDA
// 2. Get wallet information (role count, etc.)
// 3. List all roles
// 4. Get a specific role

use lazorkit_sdk::basic::wallet::LazorWallet;
use solana_sdk::pubkey::Pubkey;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Config PDA of the wallet (from create_wallet example)
    let config_pda = Pubkey::new_unique(); // Replace with actual config PDA

    // 2. Create connection (pseudo-code, implement your own SolConnection)
    // let connection = YourSolanaConnection::new("https://api.devnet.solana.com");

    println!("Fetching wallet information...");

    // 3. Fetch wallet from blockchain
    // let wallet = LazorWallet::fetch(&connection, &config_pda, None).await?;
    // println!("Wallet fetched successfully!");
    // println!("  Vault Address: {}", wallet.address);
    // println!("  Config PDA: {}", wallet.config_pda);

    // 4. Get wallet info
    // let info = wallet.fetch_info(&connection).await?;
    // println!("\nWallet Info:");
    // println!("  Total Roles: {}", info.role_count);
    // println!("  Vault Bump: {}", info.vault_bump);

    // 5. List all roles
    // let roles = wallet.list_roles(&connection).await?;
    // println!("\nRoles:");
    // for role in &roles {
    //     println!("  Role ID {}: {:?}", role.id, role.authority_type);
    //     if role.is_owner() {
    //         println!("    (Owner)");
    //     } else if role.is_admin() {
    //         println!("    (Admin)");
    //     } else {
    //         println!("    (Spender)");
    //     }
    // }

    // 6. Get specific role
    // let owner_role = wallet.get_role(0, &connection).await?;
    // println!("\nOwner role details:");
    // println!("  Type: {:?}", owner_role.authority_type);
    // if let Some(pubkey) = owner_role.ed25519_pubkey {
    //     println!("  Pubkey: {}", hex::encode(pubkey));
    // }

    println!("Example complete! (Uncomment code with real connection)");

    Ok(())
}
