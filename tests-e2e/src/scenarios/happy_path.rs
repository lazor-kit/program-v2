use crate::common::TestContext;
use anyhow::{Context, Result};
use p256::ecdsa::SigningKey;
use rand::rngs::OsRng;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
};

pub async fn run(ctx: &TestContext) -> Result<()> {
    println!("\n🚀 Running Happy Path Scenario...");

    // 1. Setup Data
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            owner_keypair.pubkey().as_ref(),
        ],
        &ctx.program_id,
    );

    println!("Wallet: {}", wallet_pda);

    // 2. Create Wallet
    println!("\n[1/3] Creating Wallet...");
    let mut data = Vec::new();
    data.push(0); // Discriminator: CreateWallet
    data.extend_from_slice(&user_seed);
    data.push(0); // Type: Ed25519
    data.push(0); // Role: Owner
    data.extend_from_slice(&[0; 6]); // Padding
    data.extend_from_slice(owner_keypair.pubkey().as_ref());

    let create_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(owner_auth_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };
    ctx.send_transaction(&[create_ix], &[&ctx.payer])
        .context("Create Wallet Failed")?;

    // 3. Fund Vault
    println!("\n[2/3] Funding Vault...");
    ctx.fund_account(&vault_pda, 10_000_000)
        .context("Fund Vault Failed")?;

    // 4. Execute Transfer (Ed25519)
    println!("\n[3/7] Executing Transfer (Ed25519)...");
    // Prepare compact instructions (System Transfer)
    let mut inner_ix_data = Vec::new();
    inner_ix_data.extend_from_slice(&2u32.to_le_bytes()); // SystemInstruction::Transfer
    inner_ix_data.extend_from_slice(&5000u64.to_le_bytes()); // Amount

    let mut compact_bytes = Vec::new();
    compact_bytes.push(0); // Program Index (SystemProgram)
    compact_bytes.push(2); // Num Accounts
    compact_bytes.push(1); // Vault (Inner Index 1)
    compact_bytes.push(2); // Payer (Inner Index 2)
    compact_bytes.extend_from_slice(&(inner_ix_data.len() as u16).to_le_bytes());
    compact_bytes.extend_from_slice(&inner_ix_data);

    let mut full_compact = Vec::new();
    full_compact.push(1); // 1 instruction
    full_compact.extend_from_slice(&compact_bytes);

    let mut exec_data = Vec::new();
    exec_data.push(4); // Discriminator: Execute
    exec_data.extend_from_slice(&full_compact);

    let execute_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(owner_auth_pda, false), // Authority
            AccountMeta::new(vault_pda, false),      // Vault (Signer)
            // Inner:
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(ctx.payer.pubkey(), false),
            // Signer for Ed25519
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
        ],
        data: exec_data,
    };
    ctx.send_transaction(&[execute_ix], &[&ctx.payer, &owner_keypair])
        .context("Execute Transfer Failed")?;

    // 5. Add Secp256r1 Authority
    println!("\n[4/7] Adding Secp256r1 Authority...");
    let rp_id = "lazorkit.vault";
    let rp_id_hash = solana_sdk::keccak::hash(rp_id.as_bytes()).to_bytes();
    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = p256::ecdsa::VerifyingKey::from(&signing_key);
    let encoded_point = verifying_key.to_encoded_point(true);
    let secp_pubkey = encoded_point.as_bytes(); // 33 bytes

    let (secp_auth_pda, _) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &rp_id_hash],
        &ctx.program_id,
    );

    let mut add_auth_data = Vec::new();
    add_auth_data.push(1); // AddAuthority
    add_auth_data.push(1); // Type: Secp256r1
    add_auth_data.push(2); // Role: Spender
    add_auth_data.extend_from_slice(&[0; 6]);
    add_auth_data.extend_from_slice(&rp_id_hash); // Seed
    add_auth_data.extend_from_slice(secp_pubkey); // Pubkey

    let add_secp_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new_readonly(owner_auth_pda, false),
            AccountMeta::new(secp_auth_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
        ],
        data: add_auth_data,
    };
    ctx.send_transaction(&[add_secp_ix], &[&ctx.payer, &owner_keypair])
        .context("Add Secp256r1 Failed")?;

    // 6. Create Session
    println!("\n[5/7] Creating Session...");
    let session_keypair = Keypair::new();
    let (session_pda, _) = Pubkey::find_program_address(
        &[
            b"session",
            wallet_pda.as_ref(),
            session_keypair.pubkey().as_ref(),
        ],
        &ctx.program_id,
    );
    let clock = ctx.client.get_epoch_info()?;
    let expires_at = clock.absolute_slot + 1000;

    let mut session_data = Vec::new();
    session_data.push(5); // CreateSession
    session_data.extend_from_slice(session_keypair.pubkey().as_ref());
    session_data.extend_from_slice(&expires_at.to_le_bytes());

    let session_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new_readonly(wallet_pda, false),
            AccountMeta::new_readonly(owner_auth_pda, false),
            AccountMeta::new(session_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
        ],
        data: session_data,
    };
    ctx.send_transaction(&[session_ix], &[&ctx.payer, &owner_keypair])
        .context("Create Session Failed")?;

    // 7. Execute via Session
    println!("\n[6/7] Executing via Session...");
    let mut session_exec_data = vec![4];
    session_exec_data.extend_from_slice(&full_compact); // Reuse transfer instruction

    let session_exec_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(session_pda, false), // Session as Authority
            AccountMeta::new(vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(ctx.payer.pubkey(), false),
            AccountMeta::new_readonly(session_keypair.pubkey(), true), // Session Signer
        ],
        data: session_exec_data,
    };
    ctx.send_transaction(&[session_exec_ix], &[&ctx.payer, &session_keypair])
        .context("Session Execute Failed")?;

    // 8. Transfer Ownership
    println!("\n[7/7] Transferring Ownership...");
    let new_owner = Keypair::new();
    let (new_owner_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            new_owner.pubkey().as_ref(),
        ],
        &ctx.program_id,
    );

    let mut transfer_own_data = Vec::new();
    transfer_own_data.push(3); // TransferOwnership
    transfer_own_data.push(0); // Ed25519
    transfer_own_data.extend_from_slice(new_owner.pubkey().as_ref());

    let transfer_ix = Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(ctx.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(owner_auth_pda, false), // Current Owner
            AccountMeta::new(new_owner_pda, false),  // New Owner
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(owner_keypair.pubkey(), true),
        ],
        data: transfer_own_data,
    };
    ctx.send_transaction(&[transfer_ix], &[&ctx.payer, &owner_keypair])
        .context("Transfer Ownership Failed")?;

    println!("✅ Happy Path Scenario Passed");
    Ok(())
}
