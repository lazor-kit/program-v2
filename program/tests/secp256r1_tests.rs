mod common;

use common::*;
use p256::ecdsa::{SigningKey, VerifyingKey};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signer::Signer,
    transaction::VersionedTransaction,
};

#[test]
// #[should_panic] // We expect this to fail due to the buffer mismatch bug
fn test_create_wallet_secp256r1_repro() {
    let mut context = setup_test();

    // 1. Generate Secp256r1 Key
    let mut rng = rand::thread_rng();
    let signing_key = SigningKey::random(&mut rng);
    let verifying_key = VerifyingKey::from(&signing_key);
    let pubkey_bytes = verifying_key.to_encoded_point(true).as_bytes().to_vec(); // 33 bytes compressed

    // Fake credential ID hash
    let credential_hash = [1u8; 32];

    let user_seed = rand::random::<[u8; 32]>();

    let (wallet_pda, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    // Authority seed for Secp256r1 is the credential hash
    let (auth_pda, _) = Pubkey::find_program_address(
        &[b"authority", wallet_pda.as_ref(), &credential_hash],
        &context.program_id,
    );

    // Build CreateWallet args
    // [user_seed(32)][type(1)][bump(1)][padding(6)] ... [credential_hash(32)][pubkey(33)]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&user_seed);
    instruction_data.push(1); // Secp256r1
    instruction_data.push(0); // bump placeholder (not verified in args parsing, just stored or ignored)
    instruction_data.extend_from_slice(&[0; 6]); // padding

    // "rest" part for Secp256r1: hash + pubkey
    instruction_data.extend_from_slice(&credential_hash);
    instruction_data.extend_from_slice(&pubkey_bytes);

    let create_wallet_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(auth_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: {
            let mut data = vec![0]; // Discriminator
            data.extend_from_slice(&instruction_data);
            data
        },
    };

    let message = v0::Message::try_compile(
        &context.payer.pubkey(),
        &[create_wallet_ix],
        &[],
        context.svm.latest_blockhash(),
    )
    .unwrap();

    let tx =
        VersionedTransaction::try_new(VersionedMessage::V0(message), &[&context.payer]).unwrap();

    // This should now SUCCEED
    context
        .svm
        .send_transaction(tx)
        .expect("CreateWallet Secp256r1 failed");
    println!("✅ Wallet created with Secp256r1 Authority");
}
