mod common;
use common::*;
use p256::ecdsa::{SigningKey, VerifyingKey};
use sha2::Digest;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Signer as SolanaSigner,
    transaction::VersionedTransaction,
};

#[test]
fn test_nonce_slot_truncation_fix() {
    let mut context = setup_test();

    // 1. Setup Wallet with Secp256r1 Authority
    let mut rng = rand::thread_rng();
    let signing_key = SigningKey::random(&mut rng);
    let verifying_key = VerifyingKey::from(&signing_key);
    let pubkey_bytes = verifying_key.to_encoded_point(true).as_bytes().to_vec(); // 33 bytes compressed

    let rp_id = "lazorkit.test";
    let rp_id_bytes = rp_id.as_bytes();
    let rp_id_len = rp_id_bytes.len() as u8;

    let mut hasher = sha2::Sha256::new();
    hasher.update(rp_id_bytes);
    let rp_id_hash = hasher.finalize();
    let credential_hash: [u8; 32] = rp_id_hash.into();

    let user_seed = rand::random::<[u8; 32]>();

    // Derive PDAs
    let (wallet_pda, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    let (auth_pda, auth_bump) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &credential_hash],
        &context.program_id,
    );

    // CreateWallet
    {
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&user_seed);
        instruction_data.push(1); // Secp256r1
        instruction_data.push(auth_bump);
        instruction_data.extend_from_slice(&[0; 6]); // padding
        instruction_data.extend_from_slice(&credential_hash);
        instruction_data.extend_from_slice(&pubkey_bytes);

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_pda, false),
                AccountMeta::new(vault_pda, false),
                AccountMeta::new(auth_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ],
            data: {
                let mut data = vec![0]; // CreateWallet
                data.extend_from_slice(&instruction_data);
                data
            },
        };

        let tx = VersionedTransaction::try_new(
            VersionedMessage::V0(
                v0::Message::try_compile(
                    &context.payer.pubkey(),
                    &[ix],
                    &[],
                    context.svm.latest_blockhash(),
                )
                .unwrap(),
            ),
            &[&context.payer],
        )
        .unwrap();
        context
            .svm
            .send_transaction(tx)
            .expect("CreateWallet failed");
    }

    // 2. Manipulate SysvarSlotHashes to simulate a specific slot history
    let current_slot = 10050u64;
    let spoof_slot = 9050u64; // Collides with 10050 if truncated by 1000

    let mut slot_hashes_data = Vec::new();
    let history_len = 512u64;
    slot_hashes_data.extend_from_slice(&history_len.to_le_bytes()); // length

    for i in 0..history_len {
        let h = current_slot - i;
        slot_hashes_data.extend_from_slice(&h.to_le_bytes());
        slot_hashes_data.extend_from_slice(&[0u8; 32]); // Dummy hashes
    }

    let slothashes_pubkey = solana_sdk::sysvar::slot_hashes::ID;
    let account = Account {
        lamports: 1,
        data: slot_hashes_data,
        owner: solana_sdk::sysvar::id(),
        executable: false,
        rent_epoch: 0,
    };
    let _ = context.svm.set_account(slothashes_pubkey, account);

    // 3. Construct Auth Payload pointing to spoof slot
    let ix_sysvar_idx = 5u8;
    let slothashes_sysvar_idx = 6u8;

    let mut authenticator_data = Vec::new();
    authenticator_data.extend_from_slice(&credential_hash); // RP ID Hash
    authenticator_data.push(0x01); // UP flag
    authenticator_data.extend_from_slice(&1u32.to_be_bytes()); // counter

    let mut auth_payload = Vec::new();
    auth_payload.extend_from_slice(&spoof_slot.to_le_bytes());
    auth_payload.push(ix_sysvar_idx);
    auth_payload.push(slothashes_sysvar_idx);
    auth_payload.push(0); // type_and_flags
    auth_payload.push(rp_id_len);
    auth_payload.extend_from_slice(rp_id_bytes);
    auth_payload.extend_from_slice(&authenticator_data);

    // 4. Construct Execute Instruction
    let mut execute_data = vec![4u8]; // Execute discriminator
    execute_data.push(0u8); // 0 compact instructions (u8)
    execute_data.extend_from_slice(&auth_payload);

    let execute_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true), // 0
            AccountMeta::new(wallet_pda, false),            // 1
            AccountMeta::new(auth_pda, false),              // 2 - Authority (Writable in Execute)
            AccountMeta::new(vault_pda, false),             // 3 - Vault
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // 4
            AccountMeta::new_readonly(solana_sdk::sysvar::instructions::id(), false), // 5
            AccountMeta::new_readonly(slothashes_pubkey, false), // 6
        ],
        data: execute_data,
    };

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(
            v0::Message::try_compile(
                &context.payer.pubkey(),
                &[execute_ix],
                &[],
                context.svm.latest_blockhash(),
            )
            .unwrap(),
        ),
        &[&context.payer],
    )
    .unwrap();

    let res = context.svm.send_transaction(tx);

    // EXPECTED: Error 3007 (InvalidSignatureAge)
    // because spoof_slot(9050) is too far from current_slot(10050)
    // even though they collide on % 1000
    assert!(res.is_err(), "Spoofed nonce should have been rejected!");
    let err = res.err().unwrap();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("Custom(3007)"),
        "Expected InvalidSignatureAge (3007) error, got: {:?}",
        err
    );
}
