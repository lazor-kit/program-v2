use anyhow::{Context, Result};
use base64::Engine;
use p256::ecdsa::SigningKey;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::env;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Comprehensive E2E Test...");

    // 1. Setup Configuration
    let rpc_url =
        env::var("RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    let keypair_path = env::var("KEYPAIR")
        .unwrap_or_else(|_| shellexpand::tilde("~/.config/solana/id.json").into_owned());
    let program_id_str =
        env::var("PROGRAM_ID").expect("Please set PROGRAM_ID environment variable.");
    let program_id = Pubkey::from_str(&program_id_str)?;

    println!("RPC: {}", rpc_url);
    println!("Payer: {}", keypair_path);
    println!("Program ID: {}", program_id);

    let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let payer = read_keypair_file(&keypair_path).expect("Failed to read keypair file");

    println!(
        "Payer Balance: {} SOL",
        client.get_balance(&payer.pubkey())? as f64 / 1_000_000_000.0
    );

    // 2. Scenario: Full Lifecycle
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    // Derive PDAs
    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &program_id);
    let (owner_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            owner_keypair.pubkey().as_ref(),
        ],
        &program_id,
    );
    println!("Wallet PDA: {}", wallet_pda);
    println!("Vault PDA: {}", vault_pda);
    println!("Owner Authority PDA: {}", owner_auth_pda);

    println!("\n--- Step 1: Create Wallet ---");
    create_wallet(
        &client,
        &payer,
        program_id,
        &user_seed,
        &owner_keypair,
        wallet_pda,
        vault_pda,
        owner_auth_pda,
    )?;

    println!("\n--- Step 2: Add Secondary Authority (Ed25519) ---");
    let secondary_keypair = Keypair::new();
    let (secondary_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            secondary_keypair.pubkey().as_ref(),
        ],
        &program_id,
    );
    println!("Secondary Authority PDA (Ed25519): {}", secondary_auth_pda);
    add_authority_ed25519(
        &client,
        &payer,
        program_id,
        wallet_pda,
        owner_auth_pda,
        secondary_auth_pda,
        &owner_keypair,
        &secondary_keypair,
    )?;

    // Fund vault first (Common setup for executions)
    let fund_ix = system_instruction::transfer(&payer.pubkey(), &vault_pda, 10_000_000); // 0.01 SOL
    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[fund_ix],
        Some(&payer.pubkey()),
        &[&payer],
        latest_blockhash,
    );
    client.send_and_confirm_transaction(&tx)?;
    println!("Vault Funded (Initial Setup).");

    println!("\n--- Step 2.5: Execute Transfer (via Secondary Authority) ---");
    execute_transfer_secondary(
        &client,
        &payer,
        program_id,
        wallet_pda,
        vault_pda,
        secondary_auth_pda,
        &secondary_keypair,
    )?;

    println!("\n--- Step 3: Execute Transfer from Vault (via Owner) ---");
    // Funds already added in previous step

    execute_transfer(
        &client,
        &payer,
        program_id,
        wallet_pda,
        vault_pda,
        owner_auth_pda,
        &owner_keypair,
    )?;

    println!("\n--- Step 4: Add Secp256r1 Authority ---");
    let rp_id = "lazorkit.vault";
    let rp_id_hash = solana_sdk::keccak::hash(rp_id.as_bytes()).to_bytes();

    // Generate Secp256r1 keys
    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = p256::ecdsa::VerifyingKey::from(&signing_key);
    let encoded_point = verifying_key.to_encoded_point(true);
    let secp_pubkey = encoded_point.as_bytes(); // 33 bytes

    let (secp_auth_pda, _) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &rp_id_hash],
        &program_id,
    );
    println!("Secp256r1 Authority PDA: {}", secp_auth_pda);

    add_authority_secp256r1(
        &client,
        &payer,
        program_id,
        wallet_pda,
        owner_auth_pda,
        secp_auth_pda,
        &owner_keypair,
        secp_pubkey,
        rp_id_hash,
    )?;

    println!("\n--- Step 4.5: Execute Transfer (via Secp256r1 Authority) ---");
    execute_transfer_secp256r1(
        &client,
        &payer,
        program_id,
        wallet_pda,
        vault_pda,
        secp_auth_pda,
        &signing_key,
        secp_pubkey,
    )?;

    println!("\n--- Step 5: Remove Secondary Authority ---");
    remove_authority(
        &client,
        &payer,
        program_id,
        wallet_pda,
        owner_auth_pda,
        secondary_auth_pda,
        &owner_keypair,
    )?;

    println!("\n--- Step 5.5: Create Session Key ---");
    let session_keypair = Keypair::new();
    let (session_pda, _) = Pubkey::find_program_address(
        &[
            b"session",
            wallet_pda.as_ref(),
            session_keypair.pubkey().as_ref(),
        ],
        &program_id,
    );
    println!("Session PDA: {}", session_pda);

    // Set expiry to 1000 slots in the future
    let clock = client.get_epoch_info()?;
    let expires_at = clock.absolute_slot + 1000;

    create_session(
        &client,
        &payer,
        program_id,
        wallet_pda,
        owner_auth_pda,
        session_pda,
        &owner_keypair,
        session_keypair.pubkey(),
        expires_at,
    )?;

    println!("\n--- Step 5.6: Execute Transfer (via Session Key) ---");
    execute_transfer_session(
        &client,
        &payer,
        program_id,
        wallet_pda,
        vault_pda,
        session_pda,
        &session_keypair,
    )?;

    println!("\n--- Step 6: Transfer Ownership ---");
    let new_owner = Keypair::new();
    let (new_owner_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            new_owner.pubkey().as_ref(),
        ],
        &program_id,
    );
    println!("New Owner Authority PDA: {}", new_owner_auth_pda);
    // Note: TransferOwnership usually adds the new owner as Admin and potentially removes the old?
    // Or just adds a new admin? The implementation details vary.
    // Assuming standard implementation: Add new authority with Owner role.

    // Actually, let's just use `transfer_ownership` instruction if it exists (Discriminator 3).
    transfer_ownership(
        &client,
        &payer,
        program_id,
        wallet_pda,
        owner_auth_pda,
        new_owner_auth_pda,
        &owner_keypair,
        &new_owner,
    )?;

    println!("\n✅ All E2E Tests Passed successfully!");

    Ok(())
}

fn create_wallet(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    user_seed: &[u8; 32],
    owner_keypair: &Keypair,
    wallet_pda: Pubkey,
    vault_pda: Pubkey,
    owner_auth_pda: Pubkey,
) -> Result<()> {
    let mut instruction_data = Vec::new();
    instruction_data.push(0u8); // Discriminator
    instruction_data.extend_from_slice(user_seed);
    instruction_data.push(0); // Type: Ed25519
    instruction_data.push(0); // Role: Owner
    instruction_data.extend_from_slice(&[0; 6]); // Padding
    instruction_data.extend_from_slice(owner_keypair.pubkey().as_ref());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(owner_auth_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: instruction_data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Create Wallet failed")?;
    println!("Wallet Created: {}", sig);
    Ok(())
}

fn add_authority_ed25519(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    authorizer_pda: Pubkey,
    new_auth_pda: Pubkey,
    authorizer_keypair: &Keypair,
    new_authority_keypair: &Keypair,
) -> Result<()> {
    // Discriminator 1: AddAuthority
    // Data: [1][Type(1)][Role(1)][Pad(6)][Seed(32)][Pubkey(32)]
    // Wait, Ed25519 is Type 0.
    let mut data = Vec::new();
    data.push(1); // Discriminator
    data.push(0); // Type: Ed25519
    data.push(1); // Role: Admin
    data.extend_from_slice(&[0; 6]);

    // For Ed25519, seed matches pubkey usually, or whatever the derivation rules are.
    // In contract: `let (auth_pda, _) = Pubkey::find_program_address(&[b"authority", wallet.as_ref(), seed], program_id);`
    // So seed is expected to be the pubkey bytes for Ed25519.
    data.extend_from_slice(new_authority_keypair.pubkey().as_ref()); // Seed
    data.extend_from_slice(new_authority_keypair.pubkey().as_ref()); // Pubkey

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wallet, false),
            AccountMeta::new_readonly(authorizer_pda, false),
            AccountMeta::new(new_auth_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(authorizer_keypair.pubkey(), true), // Sig check
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authorizer_keypair],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Add Authority (Ed25519) failed")?;
    println!("Authority Added: {}", sig);
    Ok(())
}

fn add_authority_secp256r1(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    authorizer_pda: Pubkey,
    new_auth_pda: Pubkey,
    authorizer_keypair: &Keypair,
    secp_pubkey: &[u8],
    secp_pubkey_hash: [u8; 32],
) -> Result<()> {
    let mut data = Vec::new();
    data.push(1); // Discriminator
    data.push(1); // Type: Secp256r1
    data.push(1); // Role: Admin
    data.extend_from_slice(&[0; 6]);
    data.extend_from_slice(&secp_pubkey_hash); // Seed
    data.extend_from_slice(secp_pubkey); // Pubkey (33 bytes)

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wallet, false),
            AccountMeta::new_readonly(authorizer_pda, false),
            AccountMeta::new(new_auth_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(authorizer_keypair.pubkey(), true),
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authorizer_keypair],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Add Authority (Secp256r1) failed")?;
    println!("Authority Added (Secp256r1): {}", sig);
    Ok(())
}

fn execute_transfer(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    vault: Pubkey,
    authorizer_pda: Pubkey,
    authorizer_keypair: &Keypair,
) -> Result<()> {
    // Compact: Transfer 5000 from Vault to Payer
    // Just manual serialization for test simplicity:
    // CompactInstruction { program_id_index: 2, accounts: [0, 1], data: [02, 00, 00, 00, amount...] }
    // Inner Accounts: [Vault, Payer, SystemProgram]

    // Let's assume lazorkit_program is available or we reconstruct layout.
    // Serialization for CompactInstruction is:
    // program_id_index (u16)
    // accounts_len (u16)
    // accounts (u8...)
    // data_len (u16)
    // data

    // wait, serialization in contract test used a helper. Here we do it manually.
    // Compact u16 is little endian.

    // Compact: Transfer 5000 from Vault to Payer
    // Inner Accounts construction:
    // 0: SystemProgram (at index 4 of outer)
    // 1: Vault (at index 5 of outer)
    // 2: Payer (at index 6 of outer)

    // We want to execute SystemProgram::Transfer(Vault -> Payer)
    // Program ID Index: 0 (SystemProgram)
    // Accounts: [1, 2] (Vault, Payer)

    let mut compact_bytes = Vec::new();
    compact_bytes.push(0u8); // Program Index (SystemProgram = 0)
    compact_bytes.push(2u8); // Num Accounts = 2
    compact_bytes.push(1u8); // Account 1: Vault
    compact_bytes.push(2u8); // Account 2: Payer

    let transfer_amount = 5000u64;
    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes()); // SystemInstruction::Transfer enum
    transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());

    compact_bytes.extend_from_slice(&(transfer_data.len() as u16).to_le_bytes());
    compact_bytes.extend_from_slice(&transfer_data);

    // Auth Payload: [Slot(8)][Counter(4)][SysvarIndex(1)][Sig/Data]
    // For Ed25519, we'll leave payload empty as it is ignored during simple signer verification.

    let mut full_compact = Vec::new();
    full_compact.push(1u8); // 1 instruction
    full_compact.extend_from_slice(&compact_bytes);

    let mut data = Vec::new();
    data.push(4); // Discriminator: Execute
    data.extend_from_slice(&full_compact);
    // No payload needed for Ed25519 implementation in `execute.rs` case 0.

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),  // 0: Payer (system)
            AccountMeta::new(wallet, false),         // 1: Wallet
            AccountMeta::new(authorizer_pda, false), // 2: Authority
            AccountMeta::new(vault, false),          // 3: Vault
            // Inner Accounts start here (index 4)
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // 4/Inner 0
            AccountMeta::new(vault, false),                                     // 5/Inner 1
            AccountMeta::new(payer.pubkey(), false),                            // 6/Inner 2
            // Signer for Ed25519 verify
            AccountMeta::new_readonly(authorizer_keypair.pubkey(), true), // 7 - Used by authenticate
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authorizer_keypair],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Execute failed")?;
    println!("Execute Transfer: {}", sig);
    Ok(())
}

fn execute_transfer_secondary(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    vault: Pubkey,
    authorizer_pda: Pubkey,
    authorizer_keypair: &Keypair,
) -> Result<()> {
    // Similar to execute_transfer but typically for a different authority
    // Compact: Transfer 1000 from Vault to Payer

    // Inner Accounts:
    // 0: SystemProgram
    // 1: Vault
    // 2: Payer

    let mut compact_bytes = Vec::new();
    compact_bytes.push(0u8); // Program Index (SystemProgram = 0)
    compact_bytes.push(2u8); // Num Accounts = 2
    compact_bytes.push(1u8); // Vault
    compact_bytes.push(2u8); // Payer

    let transfer_amount = 1000u64; // Smaller amount
    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes()); // SystemInstruction::Transfer
    transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());

    compact_bytes.extend_from_slice(&(transfer_data.len() as u16).to_le_bytes());
    compact_bytes.extend_from_slice(&transfer_data);

    let mut full_compact = Vec::new();
    full_compact.push(1u8);
    full_compact.extend_from_slice(&compact_bytes);

    let mut data = Vec::new();
    data.push(4); // Execute
    data.extend_from_slice(&full_compact);

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wallet, false),
            AccountMeta::new(authorizer_pda, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new(vault, false),
            AccountMeta::new(payer.pubkey(), false),
            AccountMeta::new_readonly(authorizer_keypair.pubkey(), true),
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authorizer_keypair],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Execute Transfer (Secondary) failed")?;
    println!("Execute Transfer (Secondary): {}", sig);
    Ok(())
}

fn remove_authority(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    authorizer_pda: Pubkey,
    target_pda: Pubkey,
    authorizer_keypair: &Keypair,
) -> Result<()> {
    // Discriminator: 2
    let data = vec![2];

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wallet, false),
            AccountMeta::new_readonly(authorizer_pda, false),
            AccountMeta::new(target_pda, false),
            AccountMeta::new(payer.pubkey(), false), // Refund destination
            AccountMeta::new_readonly(authorizer_keypair.pubkey(), true),
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authorizer_keypair],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Remove Authority failed")?;
    println!("Authority Removed: {}", sig);
    Ok(())
}

fn transfer_ownership(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    current_owner_pda: Pubkey,
    new_owner_pda: Pubkey,
    current_owner_keypair: &Keypair,
    new_owner_keypair: &Keypair,
) -> Result<()> {
    // Discriminator: 3
    // Data: [3][NewOwnerSeed(32)][NewOwnerPubkey(32)]
    let mut data = Vec::new();
    data.push(3);
    data.push(0); // Auth Type: Ed25519
    data.extend_from_slice(new_owner_keypair.pubkey().as_ref());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wallet, false),
            AccountMeta::new(current_owner_pda, false),
            AccountMeta::new(new_owner_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(current_owner_keypair.pubkey(), true),
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, current_owner_keypair],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Transfer Ownership failed")?;
    println!("Ownership Transferred: {}", sig);
    Ok(())
}

fn create_session(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    authorizer_pda: Pubkey,
    session_pda: Pubkey,
    authorizer_keypair: &Keypair,
    session_key: Pubkey,
    expires_at: u64,
) -> Result<()> {
    // Discriminator: 5
    // Format: [5][session_key(32)][expires_at(8)]
    let mut data = Vec::new();
    data.push(5);
    data.extend_from_slice(session_key.as_ref());
    data.extend_from_slice(&expires_at.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(wallet, false),
            AccountMeta::new_readonly(authorizer_pda, false),
            AccountMeta::new(session_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(authorizer_keypair.pubkey(), true),
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authorizer_keypair],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Create Session failed")?;
    println!("Session Created: {}", sig);
    Ok(())
}

fn execute_transfer_session(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    vault: Pubkey,
    session_pda: Pubkey,
    session_keypair: &Keypair,
) -> Result<()> {
    // Similar to execute_transfer but uses Session PDA (Discriminator 3 in the contract)
    // Compact: Transfer 2000 from Vault to Payer

    let mut compact_bytes = Vec::new();
    compact_bytes.push(0u8); // Program Index (SystemProgram = 0)
    compact_bytes.push(2u8); // Num Accounts = 2
    compact_bytes.push(1u8); // Vault
    compact_bytes.push(2u8); // Payer

    let transfer_amount = 2000u64;
    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes()); // SystemInstruction::Transfer
    transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());

    compact_bytes.extend_from_slice(&(transfer_data.len() as u16).to_le_bytes());
    compact_bytes.extend_from_slice(&transfer_data);

    let mut full_compact = Vec::new();
    full_compact.push(1u8);
    full_compact.extend_from_slice(&compact_bytes);

    let mut data = Vec::new();
    data.push(4); // Execute
    data.extend_from_slice(&full_compact);

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wallet, false),
            AccountMeta::new(session_pda, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new(vault, false),
            AccountMeta::new(payer.pubkey(), false),
            AccountMeta::new_readonly(session_keypair.pubkey(), true),
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, session_keypair],
        latest_blockhash,
    );
    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Execute Transfer (Session) failed")?;
    println!("Execute Transfer (Session): {}", sig);
    Ok(())
}

fn execute_transfer_secp256r1(
    client: &RpcClient,
    payer: &Keypair,
    program_id: Pubkey,
    wallet: Pubkey,
    vault: Pubkey,
    secp_pda: Pubkey,
    secp_key: &SigningKey,
    _secp_pubkey: &[u8],
) -> Result<()> {
    // 1. Prepare compact instructions (Transfer 1000 from Vault)
    let mut ix_bytes = Vec::new();
    ix_bytes.push(1u8); // Program Index 1 (SystemProgram)
    ix_bytes.push(2u8); // 2 accounts
    ix_bytes.push(2u8); // Account 2: Vault (at index 6 outer)
    ix_bytes.push(3u8); // Account 3: Payer (at index 7 outer)

    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes()); // SystemInstruction::Transfer
    transfer_data.extend_from_slice(&1000u64.to_le_bytes());
    ix_bytes.extend_from_slice(&(transfer_data.len() as u16).to_le_bytes());
    ix_bytes.extend_from_slice(&transfer_data);

    let mut full_compact = Vec::new();
    full_compact.push(1u8); // 1 instruction
    full_compact.extend_from_slice(&ix_bytes);

    // 2. Prepare WebAuthn mock data
    let rp_id = "lazorkit.vault";
    let rp_id_hash = solana_sdk::keccak::hash(rp_id.as_bytes()).to_bytes();

    let clock = client.get_epoch_info()?;
    let slot = clock.absolute_slot;
    let counter = 1u32;

    // Challenge = SHA256(full_compact + slot)
    let mut challenge_hasher = Sha256::new();
    challenge_hasher.update(&full_compact);
    challenge_hasher.update(&slot.to_le_bytes());
    let challenge = challenge_hasher.finalize();

    // Mock authData (37 bytes)
    let mut auth_data_mock = vec![0u8; 37];
    auth_data_mock[0..32].copy_from_slice(&rp_id_hash);
    auth_data_mock[32] = 0x01; // Flags: User Present
    auth_data_mock[33..37].copy_from_slice(&counter.to_be_bytes());

    // Reconstruct clientDataJson
    let challenge_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&challenge);
    let client_data_json = format!(
        "{{\"type\":\"webauthn.get\",\"challenge\":\"{}\",\"origin\":\"https://{}\",\"crossOrigin\":false}}",
        challenge_b64, rp_id
    );
    let mut cdj_hasher = Sha256::new();
    cdj_hasher.update(client_data_json.as_bytes());
    let client_data_hash = cdj_hasher.finalize();

    let mut signed_message = Vec::new();
    signed_message.extend_from_slice(&auth_data_mock);
    signed_message.extend_from_slice(&client_data_hash);

    // Sign with P-256
    use p256::ecdsa::signature::Signer;
    let signature: p256::ecdsa::Signature = secp_key.sign(&signed_message);
    let signature = signature.normalize_s().unwrap_or(signature);
    let signature_bytes = signature.to_bytes();

    // 3. Construct Secp256r1SigVerify instruction
    let secp_pubkey_compressed = secp_key.verifying_key().to_encoded_point(true);
    let secp_compact_bytes = secp_pubkey_compressed.as_bytes();

    let mut secp_ix_data = Vec::new();
    secp_ix_data.push(1u8); // num_signatures
    secp_ix_data.push(0u8); // padding

    let sig_verify_header_size = 2 + 14;
    let pubkey_offset = sig_verify_header_size;
    let sig_offset = pubkey_offset + 33;
    let msg_offset = sig_offset + 64;
    let msg_size = signed_message.len() as u16;

    secp_ix_data.extend_from_slice(&(sig_offset as u16).to_le_bytes());
    secp_ix_data.extend_from_slice(&0xFFFFu16.to_le_bytes());
    secp_ix_data.extend_from_slice(&(pubkey_offset as u16).to_le_bytes());
    secp_ix_data.extend_from_slice(&0xFFFFu16.to_le_bytes());
    secp_ix_data.extend_from_slice(&(msg_offset as u16).to_le_bytes());
    secp_ix_data.extend_from_slice(&msg_size.to_le_bytes());
    secp_ix_data.extend_from_slice(&0xFFFFu16.to_le_bytes());

    secp_ix_data.extend_from_slice(secp_compact_bytes);
    secp_ix_data.extend_from_slice(signature_bytes.as_slice());
    secp_ix_data.extend_from_slice(&signed_message);

    let secp_verify_ix = Instruction {
        program_id: "Secp256r1SigVerify1111111111111111111111111"
            .parse()
            .unwrap(),
        accounts: vec![],
        data: secp_ix_data,
    };

    // 4. Construct Execute instruction
    let mut auth_payload = Vec::new();
    auth_payload.extend_from_slice(&slot.to_le_bytes());
    auth_payload.push(4u8); // SysvarInstructions index (position 4)
    auth_payload.push(8u8); // SysvarSlotHashes index (position 8)
    auth_payload.push(0x10); // flags: type=get
    auth_payload.push(rp_id.len() as u8);
    auth_payload.extend_from_slice(rp_id.as_bytes());
    auth_payload.extend_from_slice(&auth_data_mock);

    let mut data = Vec::new();
    data.push(4); // Execute
    data.extend_from_slice(&full_compact);
    data.extend_from_slice(&auth_payload);

    let execute_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),   // 0
            AccountMeta::new_readonly(wallet, false), // 1
            AccountMeta::new(secp_pda, false),        // 2
            AccountMeta::new(vault, false),           // 3
            AccountMeta::new_readonly(solana_sdk::sysvar::instructions::id(), false), // 4
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // 5
            AccountMeta::new(vault, false),           // 6
            AccountMeta::new(payer.pubkey(), false),  // 7
            AccountMeta::new_readonly(solana_sdk::sysvar::slot_hashes::id(), false), // 8
        ],
        data,
    };

    let latest_blockhash = client.get_latest_blockhash()?;
    let compute_budget_ix =
        solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(400_000);

    let tx = Transaction::new_signed_with_payer(
        &[compute_budget_ix, secp_verify_ix, execute_ix],
        Some(&payer.pubkey()),
        &[payer],
        latest_blockhash,
    );

    let sig = client
        .send_and_confirm_transaction(&tx)
        .context("Execute Transfer (Secp256r1) failed")?;
    println!("Execute Transfer (Secp256r1): {}", sig);
    Ok(())
}
