use common::*;
use lazorkit_sdk::basic::actions::*;
use lazorkit_sdk::basic::wallet::LazorWallet;

use lazorkit_sdk::state::AuthorityType;
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;

mod common;

fn create_ed25519_session_data(pubkey: [u8; 32], max_session_length: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(72);
    data.extend_from_slice(&pubkey);
    data.extend_from_slice(&[0u8; 32]); // Initial session key is empty
    data.extend_from_slice(&max_session_length.to_le_bytes());
    data
}

// Override create_wallet helper to use Ed25519Session
pub fn create_wallet_with_session(
    env: &mut TestEnv,
    wallet_id: [u8; 32],
    owner_kp: &Keypair,
) -> (Pubkey, Pubkey) {
    let connection = LiteSVMConnection { svm: &env.svm };
    let owner_data = create_ed25519_session_data(owner_kp.pubkey().to_bytes(), 1000);

    let builder = CreateWalletBuilder::new()
        .with_payer(env.payer.pubkey())
        .with_owner(owner_kp.pubkey()) // Used for lookup/fallback logic but we provide explicit data
        .with_id(wallet_id)
        .with_owner_authority_type(AuthorityType::Ed25519Session)
        .with_owner_authority_key(owner_data);

    let tx = futures::executor::block_on(builder.build_transaction(&connection)).unwrap();

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();

    env.svm.send_transaction(bridge_tx(signed_tx)).unwrap();

    let (config_pda, _) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &env.program_id);
    let (vault_pda, _) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &env.program_id,
    );
    (config_pda, vault_pda)
}

#[test]
fn test_create_session_success() {
    let mut env = setup_env();
    let owner = Keypair::new();
    let wallet_id = [1u8; 32];

    // 1. Create Wallet with Ed25519Session authority
    let (config_pda, vault_pda) = create_wallet_with_session(&mut env, wallet_id, &owner);

    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Create Session Key
    let session_key = Keypair::new();
    let duration = 100;

    let builder = CreateSessionBuilder::new(&wallet)
        .with_role(0) // Owner role
        .with_session_key(session_key.pubkey().to_bytes())
        .with_duration(duration)
        .with_authorizer(owner.pubkey()); // Owner must sign

    let tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };

    let mut signed_tx = Transaction::new_unsigned(tx.message);
    signed_tx
        .try_sign(
            &[&env.payer, &owner],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_tx));
    assert!(res.is_ok(), "CreateSession failed: {:?}", res.err());
}

#[test]
fn test_execute_with_session_success() {
    let mut env = setup_env();
    let owner = Keypair::new();
    let wallet_id = [2u8; 32];

    // 1. Create Wallet
    let (config_pda, vault_pda) = create_wallet_with_session(&mut env, wallet_id, &owner);

    // Fund Vault
    env.svm
        .airdrop(
            &solana_address::Address::from(vault_pda.to_bytes()),
            1_000_000_000,
        )
        .unwrap();

    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Create Session
    let session_key = Keypair::new();
    let duration = 100;

    let create_builder = CreateSessionBuilder::new(&wallet)
        .with_role(0)
        .with_session_key(session_key.pubkey().to_bytes())
        .with_duration(duration)
        .with_authorizer(owner.pubkey());

    let create_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(
            create_builder.build_transaction(&connection, env.payer.pubkey()),
        )
        .unwrap()
    };

    let mut signed_create_tx = Transaction::new_unsigned(create_tx.message);
    signed_create_tx
        .try_sign(
            &[&env.payer, &owner],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm
        .send_transaction(bridge_tx(signed_create_tx))
        .unwrap();

    // 3. Execute with Session
    let recipient = Keypair::new();
    let transfer_ix = system_instruction::transfer(&vault_pda, &recipient.pubkey(), 500_000);

    // Auth payload: [signer_index]. Signer is session key.
    // Session key is the LAST account.
    // Accounts: [Config, Vault, System, TargetProgram(System), Vault, Recipient, Signer].
    // Indices: 0, 1, 2, 3, 4, 5, 6.
    // Signer is at index 6.
    // ExecuteBuilder automatically adds session key at end.
    // Auth payload should be index 6.
    // By default ExecuteBuilder calculates index = 3 + relative_index.
    // Relative accounts: [TargetProgram(System), Vault, Recipient, Signer].
    // Signer relative index 3.
    // 3 + 3 = 6. Correct.

    let exec_builder = ExecuteBuilder::new(&wallet)
        .with_acting_role(0)
        .add_instruction(transfer_ix)
        .with_signer(session_key.pubkey());

    let exec_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(exec_builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };

    let mut signed_exec_tx = Transaction::new_unsigned(exec_tx.message);
    // Sign with Payer and Session Key
    signed_exec_tx
        .try_sign(
            &[&env.payer, &session_key],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_exec_tx));
    assert!(res.is_ok(), "Execute with session failed: {:?}", res.err());

    // Verify balance
    let recipient_account = env.svm.get_account(&solana_address::Address::from(
        recipient.pubkey().to_bytes(),
    ));
    assert_eq!(recipient_account.unwrap().lamports, 500_000);
}

#[test]
fn test_session_expiration() {
    let mut env = setup_env();
    let owner = Keypair::new();
    let wallet_id = [3u8; 32];

    // 1. Create Wallet
    let (config_pda, vault_pda) = create_wallet_with_session(&mut env, wallet_id, &owner);
    env.svm
        .airdrop(
            &solana_address::Address::from(vault_pda.to_bytes()),
            1_000_000_000,
        )
        .unwrap();

    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 2. Create Short Session
    let session_key = Keypair::new();
    let duration = 10;

    let create_builder = CreateSessionBuilder::new(&wallet)
        .with_role(0)
        .with_session_key(session_key.pubkey().to_bytes())
        .with_duration(duration)
        .with_authorizer(owner.pubkey());

    let create_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(
            create_builder.build_transaction(&connection, env.payer.pubkey()),
        )
        .unwrap()
    };

    let mut signed_create_tx = Transaction::new_unsigned(create_tx.message);
    signed_create_tx
        .try_sign(
            &[&env.payer, &owner],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm
        .send_transaction(bridge_tx(signed_create_tx))
        .unwrap();

    // 3. Warp time forward
    let clock_account = env
        .svm
        .get_account(&solana_address::Address::from(
            solana_sdk::sysvar::clock::id().to_bytes(),
        ))
        .unwrap();
    let clock: Clock = bincode::deserialize(&clock_account.data).unwrap();
    let current_slot = clock.slot;

    env.svm.warp_to_slot(current_slot + duration + 5);

    // 4. Try Execute
    let recipient = Keypair::new();
    let transfer_ix = system_instruction::transfer(&vault_pda, &recipient.pubkey(), 500_000);

    let exec_builder = ExecuteBuilder::new(&wallet)
        .with_acting_role(0)
        .add_instruction(transfer_ix)
        .with_signer(session_key.pubkey());

    let exec_tx = {
        let connection = LiteSVMConnection { svm: &env.svm };
        futures::executor::block_on(exec_builder.build_transaction(&connection, env.payer.pubkey()))
            .unwrap()
    };

    let mut signed_exec_tx = Transaction::new_unsigned(exec_tx.message);
    signed_exec_tx
        .try_sign(
            &[&env.payer, &session_key],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();

    let res = env.svm.send_transaction(bridge_tx(signed_exec_tx));
    assert!(res.is_err(), "Execute should fail with expired session"); // This failure is expected
}
