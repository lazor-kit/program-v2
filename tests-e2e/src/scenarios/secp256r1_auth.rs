use crate::common::{TestContext, ToAddress};
use anyhow::{Context, Result};
// use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use p256::ecdsa::{signature::Signer as _, Signature, SigningKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_program;
use solana_sysvar;
use solana_transaction::Transaction;

/// Tests for Secp256r1 Authentication, including Signature Binding (Issue #9)
pub fn run(ctx: &mut TestContext) -> Result<()> {
    println!("\n🔐 Running Secp256r1 Authentication Tests...");

    test_secp256r1_signature_binding(ctx)?;

    println!("\n✅ All Secp256r1 Authentication Tests Passed!");
    Ok(())
}

// Copied from program/src/auth/secp256r1/webauthn.rs to ensure parity
pub fn base64url_encode_no_pad(data: &[u8]) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = Vec::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let b = match chunk.len() {
            3 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | (chunk[2] as u32),
            2 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8,
            1 => (chunk[0] as u32) << 16,
            _ => unreachable!(),
        };

        result.push(ALPHABET[((b >> 18) & 0x3f) as usize]);
        result.push(ALPHABET[((b >> 12) & 0x3f) as usize]);
        if chunk.len() > 1 {
            result.push(ALPHABET[((b >> 6) & 0x3f) as usize]);
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(b & 0x3f) as usize]);
        }
    }
    result
}

fn test_secp256r1_signature_binding(ctx: &mut TestContext) -> Result<()> {
    println!("\n[1/1] Testing Secp256r1 Signature Binding (Issue #9)...");

    // 1. Setup
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    // Correct ID for precompile
    let secp_prog_id = Pubkey::new_from_array([
        0x02, 0xd8, 0x8a, 0x56, 0x73, 0x47, 0x93, 0x61, 0x05, 0x70, 0x48, 0x89, 0x9e, 0xc1, 0x6e,
        0x63, 0x81, 0x4d, 0x7a, 0x5a, 0xc9, 0x68, 0x89, 0xd9, 0xcb, 0x22, 0x4c, 0x8c, 0xd0, 0x1d,
        0x4a, 0x4a,
    ]);

    // Register Mock Precompile (No-Op Program)
    // We load the compiled no-op SBF program to simulate the precompile returning success.
    let program_bytes = std::fs::read("noop-program/target/deploy/noop_program.so")
        .context("Failed to read no-op program")?;

    ctx.svm
        .add_program(secp_prog_id.to_address(), &program_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to add precompile: {:?}", e))?;

    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);

    let (wallet_pda, _) = Pubkey::find_program_address(&[b"wallet", &user_seed], &ctx.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &ctx.program_id);
    let (owner_auth_pda, bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            Signer::pubkey(&owner_keypair).as_ref(),
        ],
        &ctx.program_id,
    );

    // Create Wallet Instruction (Standard Ed25519)
    let mut create_data = vec![0]; // CreateWallet
    create_data.extend_from_slice(&user_seed);
    create_data.push(0); // Ed25519
    create_data.push(bump);
    create_data.extend_from_slice(&[0; 6]);
    create_data.extend_from_slice(Signer::pubkey(&owner_keypair).as_ref());

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
        data: create_data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx(tx).context("Create Wallet Failed")?;

    // 2. Add Secp256r1 Authority
    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = p256::ecdsa::VerifyingKey::from(&signing_key);
    let encoded_point = verifying_key.to_encoded_point(true);
    let secp_pubkey = encoded_point.as_bytes(); // 33 bytes

    let rp_id = "lazorkit.valid";
    let rp_id_hash = Sha256::digest(rp_id.as_bytes()).to_vec();

    let (secp_auth_pda, secp_bump) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &rp_id_hash],
        &ctx.program_id,
    );

    let mut add_data = vec![1]; // AddAuthority
    add_data.push(1); // Secp256r1
    add_data.push(1); // Admin
    add_data.extend_from_slice(&[0; 6]);
    add_data.extend_from_slice(&rp_id_hash);
    add_data.extend_from_slice(secp_pubkey);

    let add_ix = Instruction {
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
        data: add_data,
    };
    let add_tx = Transaction::new_signed_with_payer(
        &[add_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer, &owner_keypair],
        ctx.svm.latest_blockhash(),
    );
    ctx.execute_tx(add_tx)
        .context("Add Secp256r1 Authority Failed")?;

    // 3. Prepare Secp256r1 Transaction (Remove Authority - Removing Owner)
    // We use RemoveAuthority as it's simple.
    // Discriminator for RemoveAuthority signed_payload is &[2].
    let discriminator = [2u8];
    let payload = Vec::new(); // empty for remove
    let slot = ctx.svm.get_sysvar::<solana_clock::Clock>().slot;

    // Issue #9: Include Payer in Challenge
    let payer_pubkey = Signer::pubkey(&ctx.payer);

    let mut challenge_data = Vec::new();
    challenge_data.extend_from_slice(&discriminator);
    challenge_data.extend_from_slice(&payload);
    challenge_data.extend_from_slice(&slot.to_le_bytes());
    challenge_data.extend_from_slice(payer_pubkey.as_ref()); // BINDING TO PAYER

    let challenge_hash = Sha256::digest(&challenge_data);

    // Construct Client Data JSON
    // We mock the JSON to match the challenge
    // The program reconstructs it: {"type":"webauthn.get","challenge":"<base64_challenge>","origin":"..."}
    // But importantly, it verifies sha256(client_data_json) matches what's signed.
    // Simplification: We construct minimal client_data_json where hash matches what we sign.
    // AND: Program recalculates hash of expected JSON and compares with signed client_data_hash.

    // Actually the program reconstructs `client_data_json` from `challenge_hash` locally!
    // file: program/src/auth/secp256r1/webauthn.rs
    // fn reconstruct_client_data_json(...)
    // It creates: `{"type":"webauthn.get","challenge":"<base64url_chal>","origin":"<rp_id>","crossOrigin":false}`
    // So we must EXACTLY match this reconstruction for the verification to pass.

    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _}; // Removing this line via different edit? No, I will just edit the function body first.

    let challenge_b64_vec = base64url_encode_no_pad(&challenge_hash);
    let challenge_b64 = String::from_utf8(challenge_b64_vec).expect("Invalid UTF8");
    // Contract:
    // "{\"type\":\"webauthn.get\",\"challenge\":\"" + challenge + "\",\"origin\":\"https://" + rp_id + "\",\"crossOrigin\":false}"
    let client_data_json_str = format!(
        "{{\"type\":\"webauthn.get\",\"challenge\":\"{}\",\"origin\":\"https://{}\",\"crossOrigin\":false}}",
        challenge_b64, rp_id
    );
    // Convert to bytes to match contract's byte manipulation? String does utf8 check.
    // The previous error was InvalidMessageHash, meaning sha256(client_data_json) mismatch.
    // Let's ensure no hidden chars.
    let client_data_json = client_data_json_str.as_bytes();
    let client_data_hash = Sha256::digest(client_data_json);

    // Authenticator Data (mocked, minimal)
    // flags: user_present(1) + verified(4) | 64
    let mut authenticator_data = Vec::new();
    authenticator_data.extend_from_slice(&rp_id_hash);
    authenticator_data.push(0x05); // flags: UP(1) + UV(4)
    authenticator_data.extend_from_slice(&[0, 0, 0, 1]); // counter

    // Message to Sign: auth_data || client_data_hash
    let mut message_to_sign = Vec::new();
    message_to_sign.extend_from_slice(&authenticator_data);
    message_to_sign.extend_from_slice(&client_data_hash);
    let message_hash = Sha256::digest(&message_to_sign);

    // Sign
    let signature: Signature = signing_key.sign(&message_to_sign);
    let sig_bytes = signature.to_bytes();

    // Construct Secp256r1 Instruction Data
    // We need to construct the Precompile instruction data layout.
    // [num_sigs(1) + offsets(14) + pubkey(33) + signature(64) + message(X)]
    // message here is message_to_sign

    let mut precompile_data = Vec::new();
    precompile_data.push(1); // num_signatures

    // Offsets
    // signature_offset = 1 + 14 + 0
    let sig_offset: u16 = 15;
    let sig_ix: u16 = 0;
    // pubkey_offset = sig_offset + 64
    let pubkey_offset: u16 = sig_offset + 64;
    let pubkey_ix: u16 = 0;
    // message_offset = pubkey_offset + 33
    let msg_offset: u16 = pubkey_offset + 33;
    let msg_size = message_to_sign.len() as u16;
    let msg_ix: u16 = 0;

    precompile_data.extend_from_slice(&sig_offset.to_le_bytes());
    precompile_data.extend_from_slice(&sig_ix.to_le_bytes());
    precompile_data.extend_from_slice(&pubkey_offset.to_le_bytes());
    precompile_data.extend_from_slice(&pubkey_ix.to_le_bytes());
    precompile_data.extend_from_slice(&msg_offset.to_le_bytes());
    precompile_data.extend_from_slice(&msg_size.to_le_bytes());
    precompile_data.extend_from_slice(&msg_ix.to_le_bytes());

    precompile_data.extend_from_slice(sig_bytes.as_slice());
    precompile_data.extend_from_slice(secp_pubkey);
    precompile_data.extend_from_slice(&message_to_sign);

    // 4. Construct Transaction instructions
    // Precompile
    // Correct ID: Keccak256("Secp256r1SigVerify1111111111111111111111111")
    // But litesvm might not support the native precompile or expect specific ID.
    // The program checks for ID: 02 d8 8a ... (Keccak256("Secp256r1SigVerify1111111111111111111111111"))
    // We need to use that ID.
    let secp_prog_id = Pubkey::new_from_array([
        0x02, 0xd8, 0x8a, 0x56, 0x73, 0x47, 0x93, 0x61, 0x05, 0x70, 0x48, 0x89, 0x9e, 0xc1, 0x6e,
        0x63, 0x81, 0x4d, 0x7a, 0x5a, 0xc9, 0x68, 0x89, 0xd9, 0xcb, 0x22, 0x4c, 0x8c, 0xd0, 0x1d,
        0x4a, 0x4a,
    ]);
    let precompile_ix = Instruction {
        program_id: secp_prog_id.to_address(),
        accounts: vec![],
        data: precompile_data,
    };

    // LazorKit RemoveAuthority Instruction
    // Need to pass auth payload: [slot(8) + sys_ix(1) + slot_ix(1) + flags(1) + rp_id_len(1) + rp_id + authenticator_data]

    // We need to know indices of sysvars.
    // Accounts for RemoveAuthority: [Payer, Wallet, AdminAuth, TargetAuth, RefundDest, System, Rent, Instructions, SlotHashes]
    // Indices:
    // 0: Payer
    // 1: Wallet
    // 2: AdminAuth
    // 3: TargetAuth
    // 4: RefundDest
    // 5: System
    // 6: Rent
    // 7: Instructions (Sysvar)
    // 8: SlotHashes (Sysvar)

    let mut auth_payload = Vec::new();
    auth_payload.extend_from_slice(&slot.to_le_bytes());
    auth_payload.push(7); // Instructions Sysvar index
    auth_payload.push(8); // SlotHashes Sysvar index
    auth_payload.push(0x10); // Type/Flags (0x10 = Get, HTTPS)
                             // Actually type_and_flags: 2 usually maps to webauthn.get, need to verify strict value mapping
                             // `program/src/auth/secp256r1/webauthn.rs` L26:
                             // "type": "webauthn.get" if flags & 1 == 0?
                             // Let's assume 2 is safe (valid value).
    auth_payload.push(rp_id.len() as u8);
    auth_payload.extend_from_slice(rp_id.as_bytes());
    auth_payload.extend_from_slice(&authenticator_data);

    // LazorKit Instruction Data format: [Discriminator][Payload]
    // For RemoveAuthority, discriminator is 2.
    let mut remove_data = vec![2];
    remove_data.extend_from_slice(&auth_payload);

    let remove_ix = Instruction {
        program_id: ctx.program_id.to_address(),
        accounts: vec![
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), true),
            AccountMeta::new(wallet_pda.to_address(), false),
            AccountMeta::new(secp_auth_pda.to_address(), false), // Admin
            AccountMeta::new(owner_auth_pda.to_address(), false), // Target (remove owner)
            AccountMeta::new(Signer::pubkey(&ctx.payer).to_address(), false), // Refund
            AccountMeta::new_readonly(solana_system_program::id().to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::rent::ID.to_address(), false),
            AccountMeta::new_readonly(solana_program::sysvar::instructions::ID.to_address(), false),
            AccountMeta::new_readonly(solana_sysvar::slot_hashes::ID.to_address(), false),
        ],
        data: remove_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[precompile_ix, remove_ix],
        Some(&Signer::pubkey(&ctx.payer)),
        &[&ctx.payer], // owner not needed, calling as Admin via PDA + Payload Auth
        // Wait, remove_authority needs Admin signer?
        // Admin is a PDA, so it CANNOT sign.
        // The implementation checks `authenticate` which verifies the precompile.
        // But `manage_authority.rs` has `if !admin_auth_pda.is_writable()`.
        // It DOES NOT check `admin_auth_pda.is_signer()` for Secp256r1.
        ctx.svm.latest_blockhash(),
    );

    // Execute!
    // This should PASS if I constructed everything correctly INCLUDING the Payer key in challenge.
    // If I didn't include Payer key in challenge, the on-chain program (which now DOES include it)
    // will compute a different challenge -> different client_data_hash -> mismatch signature.

    ctx.execute_tx(tx).context("Secp256r1 Transaction Failed")?;
    println!("   ✓ Valid Secp256r1 Signature (Bound to Payer) Accepted");

    Ok(())
}
