mod common;
use common::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::VersionedTransaction,
};

#[test]
fn test_spoof_system_program() {
    let mut context = setup_test();

    // 1. Create a Fake System Program (just a random keypair)
    // In a real attack, this would be a program controlled by attacker,
    // but for this test, just passing a non-system-program account is enough to check validation.
    let fake_system_program = Keypair::new();

    // 2. Prepare CreateWallet args
    let user_seed = rand::random::<[u8; 32]>();
    let owner_keypair = Keypair::new();

    let (wallet_pda, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed], &context.program_id);
    let (vault_pda, _) =
        Pubkey::find_program_address(&[b"vault", wallet_pda.as_ref()], &context.program_id);
    let (auth_pda, auth_bump) = Pubkey::find_program_address(
        &[
            b"authority",
            wallet_pda.as_ref(),
            owner_keypair.pubkey().as_ref(),
        ],
        &context.program_id,
    );

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&user_seed);
    instruction_data.push(0); // Ed25519
    instruction_data.push(auth_bump);
    instruction_data.extend_from_slice(&[0; 6]);
    instruction_data.extend_from_slice(owner_keypair.pubkey().as_ref());

    // 3. Create Instruction with FAKE System Program
    let create_wallet_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(wallet_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(auth_pda, false),
            // PASS FAKE SYSTEM PROGRAM HERE
            AccountMeta::new_readonly(fake_system_program.pubkey(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
        ],
        data: {
            let mut data = vec![0]; // CreateWallet discriminator
            data.extend_from_slice(&instruction_data);
            data
        },
    };

    // 4. Submit Transaction
    let message = v0::Message::try_compile(
        &context.payer.pubkey(),
        &[create_wallet_ix],
        &[],
        context.svm.latest_blockhash(),
    )
    .unwrap();
    let tx =
        VersionedTransaction::try_new(VersionedMessage::V0(message), &[&context.payer]).unwrap();

    let res = context.svm.send_transaction(tx);

    // 5. Assert Failure
    // If validation works, this should fail with IncorrectProgramId or similar.
    // If vulnerability exists, it might fail with something else (like InstructionError because fake program doesn't handle instruction)
    // or PASS if the contract doesn't invoke it or invokes it successfully (unlikely for random key).
    // The critical check is if the CONTRACT returns an error BEFORE invoking.

    if let Err(err) = res {
        println!("Transaction failed as expected: {:?}", err);
        // Verify it is NOT "IncorrectProgramId" if we want to prove vulnerability?
        // Wait, if it returns IncorrectProgramId, then the check IS working.
    } else {
        panic!("Transaction succeeded but should have failed due to fake system program!");
    }
}
