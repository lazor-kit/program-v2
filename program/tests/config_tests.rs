mod common;

use common::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};

#[test]
fn test_config_lifecycle() {
    let mut context = setup_test();

    // 1. Verify Config PDA was initialized in setup_test
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], &context.program_id);
    let config_account = context.svm.get_account(&config_pda);
    assert!(config_account.is_some(), "Config should be initialized");

    // 2. Update Config (Normal case)
    let new_admin = Keypair::new();
    let new_wallet_fee = 50000u64;
    let new_action_fee = 5000u64;
    
    // args: update_wallet_fee(1), update_action_fee(1), update_num_shards(1), update_admin(1), num_shards(16)
    // padding(3), wallet_fee(8), action_fee(8), admin(32)
    let mut update_data = vec![7]; // discriminator
    update_data.extend_from_slice(&[1, 1, 1, 1, 16]); // updates + num_shards
    update_data.extend_from_slice(&[0, 0, 0]); // padding
    update_data.extend_from_slice(&new_wallet_fee.to_le_bytes());
    update_data.extend_from_slice(&new_action_fee.to_le_bytes());
    update_data.extend_from_slice(new_admin.pubkey().as_ref());

    let update_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
        ],
        data: update_data,
    };

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(v0::Message::try_compile(
            &context.payer.pubkey(),
            &[update_ix],
            &[],
            context.svm.latest_blockhash(),
        ).unwrap()),
        &[&context.payer],
    ).unwrap();

    context.svm.send_transaction(tx).expect("UpdateConfig failed");
    println!("✅ Config updated successfully");

    // 3. Reject update from unauthorized payer (non-admin)
    let hacker = Keypair::new();
    context.svm.airdrop(&hacker.pubkey(), 1_000_000_000).unwrap();

    let mut update_data_hacker = vec![7];
    update_data_hacker.extend_from_slice(&[1, 0, 0, 0, 16, 0, 0, 0]);
    update_data_hacker.extend_from_slice(&999999u64.to_le_bytes());
    update_data_hacker.extend_from_slice(&0u64.to_le_bytes());
    update_data_hacker.extend_from_slice(&[0; 32]);

    let update_ix_hacker = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(hacker.pubkey(), true),
            AccountMeta::new(config_pda, false),
        ],
        data: update_data_hacker,
    };

    let tx_hacker = VersionedTransaction::try_new(
        VersionedMessage::V0(v0::Message::try_compile(
            &hacker.pubkey(),
            &[update_ix_hacker],
            &[],
            context.svm.latest_blockhash(),
        ).unwrap()),
        &[&hacker],
    ).unwrap();

    let res = context.svm.send_transaction(tx_hacker);
    assert!(res.is_err(), "Hacker should not be able to update config");
    println!("✅ Unauthorized update rejected");

    // 4. Test InitTreasuryShard
    let shard_id = 5;
    let shard_id_bytes = [shard_id];
    let (shard_pda, _) = Pubkey::find_program_address(&[b"treasury", &shard_id_bytes], &context.program_id);

    let init_shard_ix = Instruction {
        program_id: context.program_id,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(shard_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
        ],
        data: vec![11, shard_id],
    };

    let tx_shard = VersionedTransaction::try_new(
        VersionedMessage::V0(v0::Message::try_compile(
            &context.payer.pubkey(),
            &[init_shard_ix],
            &[],
            context.svm.latest_blockhash(),
        ).unwrap()),
        &[&context.payer],
    ).unwrap();

    context.svm.send_transaction(tx_shard).expect("InitTreasuryShard failed");
    assert!(context.svm.get_account(&shard_pda).is_some(), "Shard should exist");
    println!("✅ Treasury shard initialized");
}
