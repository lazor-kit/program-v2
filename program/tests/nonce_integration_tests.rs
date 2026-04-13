mod common;
use common::*;
use p256::ecdsa::{SigningKey, VerifyingKey};
use sha2::Digest;
use solana_sdk::{
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

    let mut hasher = sha2::Sha256::new();
    hasher.update(rp_id_bytes);
    let rp_id_hash = hasher.finalize();
    let credential_hash: [u8; 32] = rp_id_hash.into();

    let credential_id_hash = [5u8; 32];
    let user_seed = rand::random::<[u8; 32]>();

    // Derive PDAs
    let (wallet_pda, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    let (auth_pda, auth_bump) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &credential_id_hash],
        &context.program_id,
    );

    // CreateWallet — now includes rpId in instruction data
    {
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&user_seed);
        instruction_data.push(1); // Secp256r1
        instruction_data.push(auth_bump);
        instruction_data.extend_from_slice(&[0; 6]); // padding
        instruction_data.extend_from_slice(&credential_id_hash);
        instruction_data.extend_from_slice(&pubkey_bytes);
        instruction_data.push(rp_id_bytes.len() as u8); // rpIdLen
        instruction_data.extend_from_slice(rp_id_bytes); // rpId

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

    // 2. Warp the clock forward so slot 0 is >150 slots in the past
    {
        let mut clock = context.svm.get_sysvar::<solana_sdk::clock::Clock>();
        clock.slot = 200; // Well past MAX_SLOT_AGE (150)
        context.svm.set_sysvar(&clock);
    }

    // Construct Auth Payload with a slot that's too old
    // Clock::get() returns current_slot (200). We submit slot 0, which is >150 slots ago.
    let spoof_slot = 0u64; // Slot 0 is >150 slots behind current_slot (200)
    let ix_sysvar_idx = 4u8; // sysvar_instructions is at index 4 (no slothashes anymore)

    let mut authenticator_data = Vec::new();
    authenticator_data.extend_from_slice(&credential_hash); // RP ID Hash
    authenticator_data.push(0x01); // UP flag
    authenticator_data.extend_from_slice(&1u32.to_be_bytes()); // WebAuthn counter

    // Odometer counter: first use expects 1 (stored counter is 0)
    let odometer_counter: u32 = 1;

    // New auth payload layout: [slot(8)][counter(4)][sysvarIxIdx(1)][flags(1)][authenticatorData(M)]
    let mut auth_payload = Vec::new();
    auth_payload.extend_from_slice(&spoof_slot.to_le_bytes()); // slot
    auth_payload.extend_from_slice(&odometer_counter.to_le_bytes()); // u32 counter
    auth_payload.push(ix_sysvar_idx); // sysvar instructions index
    auth_payload.push(0); // type_and_flags
    auth_payload.extend_from_slice(&authenticator_data);

    // 3. Construct Execute Instruction (no SlotHashes account needed)
    let mut execute_data = vec![4u8]; // Execute discriminator
    execute_data.push(0u8); // 0 compact instructions (u8)
    execute_data.extend_from_slice(&auth_payload);

    let execute_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),           // 0
            AccountMeta::new(wallet_pda, false),                       // 1
            AccountMeta::new(auth_pda, false),                         // 2 - Authority (Writable)
            AccountMeta::new(vault_pda, false),                        // 3 - Vault
            AccountMeta::new_readonly(solana_sdk::sysvar::instructions::id(), false), // 4
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
    // because spoof_slot(0) is too far from current_slot
    assert!(res.is_err(), "Spoofed nonce should have been rejected!");
    let err = res.err().unwrap();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("Custom(3007)"),
        "Expected InvalidSignatureAge (3007) error, got: {:?}",
        err
    );
}
