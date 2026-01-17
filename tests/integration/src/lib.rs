// use lazorkit_program::processor::process_instruction;
use lazorkit_state::authority::{ed25519::Ed25519Authority, AuthorityType};
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

// Export dependencies for convenience in tests
pub use lazorkit_program;
pub use lazorkit_state;
pub use solana_program_test;
pub use solana_sdk;

pub async fn setup_test_context() -> (ProgramTestContext, Keypair, Pubkey) {
    let program_id_str = "LazorKit11111111111111111111111111111111111";
    let program_id = program_id_str.parse().unwrap();

    // Link directly to Rust code to avoid stale SBF binaries
    let program_test = ProgramTest::new("lazorkit_program", program_id, None);

    // Can add plugins as well if needed
    // program_test.add_program("lazorkit_sol_limit_plugin", lazorkit_sol_limit_plugin::id(), processor!(...));

    let context = program_test.start_with_context().await;
    let payer = Keypair::from_bytes(&context.payer.to_bytes()).unwrap(); // Clone payer

    (context, payer, program_id)
}

pub async fn create_wallet_helper(
    context: &mut ProgramTestContext,
    program_id: Pubkey,
    payer: &Keypair,
    wallet_id: [u8; 32],
    owner_keypair: &Keypair,
) -> (Pubkey, Pubkey, u8, u8) {
    let (config_pda, bump) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &program_id);
    let (wallet_address, wallet_bump) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &program_id,
    );

    let authority_data = Ed25519Authority::new(owner_keypair.pubkey().to_bytes());
    use lazorkit_state::IntoBytes;
    let auth_blob = authority_data.into_bytes().unwrap().to_vec();

    let mut instruction_data = vec![];
    instruction_data.extend_from_slice(&wallet_id);
    instruction_data.push(bump);
    instruction_data.push(wallet_bump);
    instruction_data.extend_from_slice(&(AuthorityType::Ed25519 as u16).to_le_bytes());
    // owner_data length (u32)
    instruction_data.extend_from_slice(&(auth_blob.len() as u32).to_le_bytes());
    instruction_data.extend_from_slice(&auth_blob);

    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(config_pda, false),
        solana_sdk::instruction::AccountMeta::new(payer.pubkey(), true),
        solana_sdk::instruction::AccountMeta::new(wallet_address, false),
        solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];

    let instruction = Instruction {
        program_id,
        accounts,
        data: vec![0].into_iter().chain(instruction_data).collect(), // 0 = CreateWallet
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        context.last_blockhash,
    );

    context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    (config_pda, wallet_address, bump, wallet_bump)
}
