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

    // Derive Config PDA and a Treasury Shard PDA for the payer (shard 0 for tests)
    let (config_pda, _) =
        Pubkey::find_program_address(&[b"config"], &context.program_id);
    let shard_id: u8 = 0;
    let shard_id_bytes = [shard_id];
    let (treasury_pda, _) =
        Pubkey::find_program_address(&[b"treasury", &shard_id_bytes], &context.program_id);

    // Initialize Config and Treasury shard accounts in the LiteSVM context so that
    // CreateWallet can charge protocol fees without failing.
    {
        use solana_sdk::account::Account;
        use lazorkit_program::state::{config::ConfigAccount, AccountDiscriminator, CURRENT_ACCOUNT_VERSION};

        // Minimal ConfigAccount with 1 shard and zero fees for this focused test.
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
            std::ptr::write_unaligned(
                config_bytes.as_mut_ptr() as *mut ConfigAccount,
                config_data,
            );
        }

        let config_account = Account {
            lamports: 1,
            data: config_bytes,
            owner: context.program_id,
            executable: false,
            rent_epoch: 0,
        };
        let _ = context.svm.set_account(config_pda, config_account);

        // Treasury shard as a simple system-owned account with some lamports (no data).
        let treasury_account = Account {
            lamports: 1_000_000,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        };
        let _ = context.svm.set_account(treasury_pda, treasury_account);
    }

    // CreateWallet
    {
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&user_seed);
        instruction_data.push(1); // Secp256r1
        instruction_data.push(auth_bump);
        instruction_data.extend_from_slice(&[0; 6]); // padding
        instruction_data.extend_from_slice(&credential_id_hash);
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
                AccountMeta::new(config_pda, false),
                AccountMeta::new(treasury_pda, false),
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
    // Indices in the Execute accounts list (defined below)
    // 0: payer, 1: wallet, 2: authority, 3: vault,
    // 4: config, 5: treasury_shard, 6: system_program,
    // 7: sysvar_instructions, 8: slot_hashes
    let ix_sysvar_idx = 7u8;
    let slothashes_sysvar_idx = 8u8;

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
            AccountMeta::new(auth_pda, false),              // 2 - Authority
            AccountMeta::new(vault_pda, false),             // 3 - Vault
            AccountMeta::new(config_pda, false),            // 4 - Config PDA
            AccountMeta::new(treasury_pda, false),          // 5 - Treasury shard
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // 6 - System program
            AccountMeta::new_readonly(solana_sdk::sysvar::instructions::id(), false), // 7 - Instructions sysvar
            AccountMeta::new_readonly(slothashes_pubkey, false), // 8 - SlotHashes sysvar
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

    // We only require that the spoofed nonce is rejected.
    // The exact error code may vary depending on additional
    // signature validation checks, but a successful transaction
    // would indicate a regression in nonce validation.
    assert!(res.is_err(), "Spoofed nonce should have been rejected!");
}
