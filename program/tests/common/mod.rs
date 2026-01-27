use litesvm::LiteSVM;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

pub struct TestContext {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub program_id: Pubkey,
}

pub fn setup_test() -> TestContext {
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();

    // Airdrop to payer
    svm.airdrop(&payer.pubkey(), 10_000_000_000)
        .expect("Failed to airdrop");

    // Load program
    let program_id = load_program(&mut svm);

    TestContext {
        svm,
        payer,
        program_id,
    }
}

fn load_program(svm: &mut LiteSVM) -> Pubkey {
    // LazorKit program ID (deterministic for tests)
    let program_id = Pubkey::new_unique();

    // Load the compiled program
    let path = "../target/deploy/lazorkit_program.so";
    svm.add_program_from_file(program_id, path)
        .expect("Failed to load program");

    program_id
}
