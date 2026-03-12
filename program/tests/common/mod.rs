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

    // Initialize a zero-fee Config PDA and a single Treasury shard (id 0)
    // so that protocol fee logic in tests has valid accounts to read from.
    {
        use lazorkit_program::state::{config::ConfigAccount, AccountDiscriminator, CURRENT_ACCOUNT_VERSION};
        use solana_sdk::account::Account;

        let (config_pda, _) =
            Pubkey::find_program_address(&[b"config"], &program_id);
        let shard_id: u8 = 0;
        let shard_id_bytes = [shard_id];
        let (treasury_pda, _) =
            Pubkey::find_program_address(&[b"treasury", &shard_id_bytes], &program_id);

        let config_data = ConfigAccount {
            discriminator: AccountDiscriminator::Config as u8,
            bump: 0,
            version: CURRENT_ACCOUNT_VERSION,
            num_shards: 1,
            _padding: [0; 4],
            admin: payer.pubkey().to_bytes().into(),
            wallet_fee: 0,
            action_fee: 0,
        };

        let mut config_bytes = vec![0u8; std::mem::size_of::<ConfigAccount>()];
        unsafe {
            std::ptr::write_unaligned(
                config_bytes.as_mut_ptr() as *mut ConfigAccount,
                config_data,
            );
        }

        let config_account = Account {
            lamports: 100_000_000, // Enough for rent
            data: config_bytes,
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        };
        let _ = svm.set_account(config_pda, config_account);

        let treasury_account = Account {
            lamports: 100_000_000,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        };
        let _ = svm.set_account(treasury_pda, treasury_account);
    }

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
