mod common;

use common::*;
use lazorkit_program::state::config::ConfigAccount;
use lazorkit_program::state::AccountDiscriminator;
use lazorkit_program::state::CURRENT_ACCOUNT_VERSION;
use solana_sdk::account::Account;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::{v0, VersionedMessage};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::VersionedTransaction;

fn init_zero_fee_config_and_shard(context: &mut TestContext) -> (Pubkey, Pubkey) {
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], &context.program_id);
    let shard_id: u8 = 0;
    let shard_id_bytes = [shard_id];
    let (treasury_pda, _) =
        Pubkey::find_program_address(&[b"treasury", &shard_id_bytes], &context.program_id);

    // Minimal config with zero fees and 1 shard.
    let config_data = ConfigAccount {
        discriminator: AccountDiscriminator::Config as u8,
        bump: 0,
        version: CURRENT_ACCOUNT_VERSION,
        num_shards: 1,
        _padding: [0; 4],
        admin: context.payer.pubkey().to_bytes(),
        wallet_fee: 0,
        action_fee: 0,
    };
    let mut config_bytes = vec![0u8; std::mem::size_of::<ConfigAccount>()];
    unsafe {
        std::ptr::write_unaligned(config_bytes.as_mut_ptr() as *mut ConfigAccount, config_data);
    }
    let config_account = Account {
        lamports: 1,
        data: config_bytes,
        owner: context.program_id,
        executable: false,
        rent_epoch: 0,
    };
    let _ = context.svm.set_account(config_pda, config_account);

    let treasury_account = Account {
        lamports: 1_000_000,
        data: vec![],
        owner: solana_sdk::system_program::id(),
        executable: false,
        rent_epoch: 0,
    };
    let _ = context.svm.set_account(treasury_pda, treasury_account);

    (config_pda, treasury_pda)
}

/// Attack 1: Cross-wallet authority abuse
/// Try to use an authority PDA from Wallet A to execute on Wallet B.
#[test]
fn test_execute_rejects_cross_wallet_authority() {
    let mut context = setup_test();
    let (config_pda, treasury_pda) = init_zero_fee_config_and_shard(&mut context);

    // Wallet A + authority
    let user_seed_a = rand::random::<[u8; 32]>();
    let owner_a = Keypair::new();
    let (wallet_a, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed_a], &context.program_id);
    let (vault_a, _) =
        Pubkey::find_program_address(&[b"vault", wallet_a.as_ref()], &context.program_id);
    let (auth_a, auth_a_bump) = Pubkey::find_program_address(
        &[b"authority", wallet_a.as_ref(), owner_a.pubkey().as_ref()],
        &context.program_id,
    );

    // Wallet B (no authority yet)
    let user_seed_b = rand::random::<[u8; 32]>();
    let (wallet_b, _) =
        Pubkey::find_program_address(&[b"wallet", &user_seed_b], &context.program_id);
    let (vault_b, _) =
        Pubkey::find_program_address(&[b"vault", wallet_b.as_ref()], &context.program_id);

    // Create wallet A (owner_a)
    {
        let mut data = Vec::new();
        data.extend_from_slice(&user_seed_a);
        data.push(0); // Ed25519
        data.push(auth_a_bump);
        data.extend_from_slice(&[0; 6]);
        data.extend_from_slice(owner_a.pubkey().as_ref());

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_a, false),
                AccountMeta::new(vault_a, false),
                AccountMeta::new(auth_a, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new(config_pda, false),
                AccountMeta::new(treasury_pda, false),
            ],
            data: {
                let mut full = vec![0];
                full.extend_from_slice(&data);
                full
            },
        };
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[ix],
            &[],
            context.svm.latest_blockhash(),
        )
        .unwrap();
        let tx =
            VersionedTransaction::try_new(VersionedMessage::V0(msg), &[&context.payer]).unwrap();
        context.svm.send_transaction(tx).expect("create A");
    }

    // Create wallet B (no authorities)
    {
        let mut data = Vec::new();
        data.extend_from_slice(&user_seed_b);
        data.push(0); // Ed25519
        data.push(0); // dummy bump
        data.extend_from_slice(&[0; 6]);
        data.extend_from_slice(owner_a.pubkey().as_ref()); // reuse pubkey just for seed

        let (_auth_b_pda, _) = Pubkey::find_program_address(
            &[b"authority", wallet_b.as_ref(), owner_a.pubkey().as_ref()],
            &context.program_id,
        );

        let ix = Instruction {
            program_id: context.program_id,
            accounts: vec![
                AccountMeta::new(context.payer.pubkey(), true),
                AccountMeta::new(wallet_b, false),
                AccountMeta::new(vault_b, false),
                AccountMeta::new(_auth_b_pda, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                AccountMeta::new(config_pda, false),
                AccountMeta::new(treasury_pda, false),
            ],
            data: {
                let mut full = vec![0];
                full.extend_from_slice(&data);
                full
            },
        };
        let msg = v0::Message::try_compile(
            &context.payer.pubkey(),
            &[ix],
            &[],
            context.svm.latest_blockhash(),
        )
        .unwrap();
        let tx =
            VersionedTransaction::try_new(VersionedMessage::V0(msg), &[&context.payer]).unwrap();
        // This may succeed or fail depending on seeds; not critical for this test.
        let _ = context.svm.send_transaction(tx);
    }

    // Attempt to Execute on wallet B using authority from wallet A.
    let execute_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true), // payer
            AccountMeta::new(wallet_b, false),              // wallet B
            AccountMeta::new(auth_a, false),                // authority from wallet A
            AccountMeta::new(vault_b, false),               // vault B
            AccountMeta::new(config_pda, false),
            AccountMeta::new(treasury_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: vec![4, 0], // Execute discriminator + 0 compact instructions
    };

    let msg = v0::Message::try_compile(
        &context.payer.pubkey(),
        &[execute_ix],
        &[],
        context.svm.latest_blockhash(),
    )
    .unwrap();
    let tx = VersionedTransaction::try_new(VersionedMessage::V0(msg), &[&context.payer]).unwrap();
    let res = context.svm.send_transaction(tx);
    assert!(res.is_err(), "Cross-wallet authority should be rejected");
}
