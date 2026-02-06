use crate::common::{TestContext, ToAddress};
use anyhow::{Context, Result};
use p256::ecdsa::SigningKey;
use rand::rngs::OsRng;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_program::hash::hash;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_program;
use solana_sysvar;
use solana_transaction::Transaction;

pub fn run(ctx: &mut TestContext) -> Result<()> {
    println!("\n🚀 Running Happy Path Scenario...");

    // 1. Setup Data
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, auth_bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(),
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
    data.push(auth_bump); // auth_bump from find_program_address
    data.extend_from_slice(&[0; 6]); // Padding
    data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());

    let create_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
        ],
        data,
    };

    let message_create = Message::new(&[create_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut create_tx = Transaction::new_unsigned(message_create);
    create_tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    ctx.execute_tx(create_tx)?;
    println!("   ✓ Wallet created");

    // 3. Fund Vault - using manual transfer instruction construction
    println!("\n[Test] Funding Vault...");
    let mut transfer_data = Vec::new();
    transfer_data.extend_from_slice(&2u32.to_le_bytes()); // Transfer instruction
    transfer_data.extend_from_slice(&1_000_000_000u64.to_le_bytes());

    let fund_ix = Instruction {
        program_id: solana_system_program::id().to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(vault_pda.to_address(), false),
        ],
        data: transfer_data,
    };

    let message_fund = Message::new(&[fund_ix], Some(&Signer::pubkey(&ctx.payer).to_address()));
    let mut fund_tx = Transaction::new_unsigned(message_fund);
    fund_tx.sign(&[&ctx.payer], ctx.svm.latest_blockhash());

    ctx.execute_tx(fund_tx)?;

    // 4. Execute Transfer (Ed25519)
    println!("\n[3/7] Executing Transfer (Ed25519)...");
    // Prepare compact instructions (System Transfer)
    let mut inner_ix_data = Vec::new();
    inner_ix_data.extend_from_slice(&2u32.to_le_bytes()); // SystemInstruction::Transfer
    inner_ix_data.extend_from_slice(&5000u64.to_le_bytes()); // Amount

    // Account indices in execute accounts list:
    // 0: payer, 1: wallet_pda, 2: authority, 3: vault
    // 4: system_program, 5: vault (inner), 6: payer (inner), 7: owner signer
    let mut compact_bytes = Vec::new();
    compact_bytes.push(4); // Program Index = system_program (index 4)
    compact_bytes.push(2); // Num Accounts
    compact_bytes.push(5); // Vault (inner) - index 5
    compact_bytes.push(6); // Payer (inner) - index 6
    compact_bytes.extend_from_slice(&(inner_ix_data.len() as u16).to_le_bytes());
    compact_bytes.extend_from_slice(&inner_ix_data);

    let mut full_compact = Vec::new();
    full_compact.push(1); // 1 instruction
    full_compact.extend_from_slice(&compact_bytes);

    let mut exec_data = Vec::new();
    exec_data.push(4); // Discriminator: Execute
    exec_data.extend_from_slice(&full_compact);

    let execute_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false), // Authority
            AccountMeta::new(vault_pda.to_address(), false),      // Vault (Signer)
            // Inner:
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), false),
            // Signer for Ed25519
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: exec_data,
    };

    let message_exec = Message::new(
        &[execute_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut execute_tx = Transaction::new_unsigned(message_exec);
    execute_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(execute_tx)
        .context("Execute Transfer Failed")?;

    // 5. Add Secp256r1 Authority
    println!("\n[4/7] Adding Secp256r1 Authority...");
    let rp_id = "lazorkit.vault";
    let rp_id_hash = hash(rp_id.as_bytes()).to_bytes();
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
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new_readonly(owner_auth_pda.to_address(), false),
            AccountMeta::new(secp_auth_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: add_auth_data,
    };

    let message_add = Message::new(
        &[add_secp_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut add_spender_tx = Transaction::new_unsigned(message_add);
    add_spender_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(add_spender_tx)
        .context("Add Secp256r1 Failed")?;

    // 6. Create Session
    println!("\n[5/7] Creating Session...");
    let session_keypair = Keypair::new();
    let (session_pda, _) = Pubkey::find_program_address(
        &[
            b"session",
            wallet_pda.as_ref(),
            Signer::pubkey(&session_keypair).as_ref(),
        ],
        &ctx.program_id,
    );
    let clock: solana_clock::Clock = ctx.svm.get_sysvar();
    let expires_at = clock.slot + 1000;

    let mut session_data = Vec::new();
    session_data.push(5); // CreateSession
    session_data.extend_from_slice(Signer::pubkey(&session_keypair).as_ref());
    session_data.extend_from_slice(&expires_at.to_le_bytes());

    let session_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new_readonly(wallet_pda.to_address(), false),
            AccountMeta::new_readonly(owner_auth_pda.to_address(), false),
            AccountMeta::new(session_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: session_data,
    };

    let message_session = Message::new(
        &[session_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut session_tx = Transaction::new_unsigned(message_session);
    session_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(session_tx)
        .context("Create Session Failed")?;

    // 7. Execute via Session
    println!("\n[6/7] Executing via Session...");
    let mut session_exec_data = vec![4];
    session_exec_data.extend_from_slice(&full_compact); // Reuse transfer instruction

    let session_exec_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(session_pda.to_address(), false), // Session as Authority
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new(vault_pda.to_address(), false),
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&session_keypair).to_address(), true), // Session Signer
        ],
        data: session_exec_data,
    };

    let message_sess_exec = Message::new(
        &[session_exec_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut session_exec_tx = Transaction::new_unsigned(message_sess_exec);
    session_exec_tx.sign(&[&ctx.payer, &session_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(session_exec_tx)
        .context("Session Execute Failed")?;

    // 8. Transfer Ownership
    println!("\n[7/7] Transferring Ownership...");
    let new_owner = Keypair::new();
    let (new_owner_pda, _) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&new_owner).as_ref(),
        ],
        &ctx.program_id,
    );

    let mut transfer_own_data = Vec::new();
    transfer_own_data.push(3); // TransferOwnership
    transfer_own_data.push(0); // Ed25519
    transfer_own_data.extend_from_slice(Signer::pubkey(&new_owner).as_ref());

    let transfer_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(owner_auth_pda.to_address(), false), // Current Owner
            AccountMeta::new(new_owner_pda.to_address(), false),  // New Owner
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(Signer::pubkey(&owner_keypair).to_address(), true),
        ],
        data: transfer_own_data,
    };

    let message_transfer = Message::new(
        &[transfer_ix],
        Some(&Signer::pubkey(&ctx.payer).to_address()),
    );
    let mut transfer_tx = Transaction::new_unsigned(message_transfer);
    transfer_tx.sign(&[&ctx.payer, &owner_keypair], ctx.svm.latest_blockhash());

    ctx.execute_tx(transfer_tx)
        .context("Transfer Ownership Failed")?;

    println!("✅ Happy Path Scenario Passed");
    Ok(())
}
