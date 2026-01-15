mod common;
use common::{get_program_path, setup_env, TestEnv};

use lazorkit_program::instruction::LazorKitInstruction;
use lazorkit_state::{
    authority::{ed25519::Ed25519Authority, AuthorityType},
    LazorKitWallet, Position,
};
use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};
use std::path::PathBuf;

#[test]
fn test_create_wallet_success() {
    let mut env = setup_env();
    let payer = env.payer;
    let mut svm = env.svm;
    let program_id = env.program_id;
    let program_id_pubkey = env.program_id; // Same type now

    let wallet_id = [7u8; 32];
    let (config_pda, bump) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &program_id);
    let (vault_pda, wallet_bump) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &program_id,
    );

    println!("Config PDA: {}", config_pda);
    println!("Vault PDA: {}", vault_pda);

    // Prepare Instruction Data
    let owner_keypair = Keypair::new();
    let authority_data = Ed25519Authority::new(owner_keypair.pubkey().to_bytes());
    use lazorkit_state::IntoBytes;
    let auth_blob = authority_data
        .into_bytes()
        .expect("Failed to serialize auth")
        .to_vec();

    let instruction = LazorKitInstruction::CreateWallet {
        id: wallet_id,
        bump,
        wallet_bump,
        owner_authority_type: AuthorityType::Ed25519 as u16,
        owner_authority_data: auth_blob.clone(),
    };

    let instruction_data = borsh::to_vec(&instruction).unwrap();

    let system_program_id = system_program::id();

    let accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: system_program_id,
            is_signer: false,
            is_writable: false,
        },
    ];

    let create_ix = Instruction {
        program_id,
        accounts,
        data: instruction_data,
    };

    // Execute Transaction
    let transaction = Transaction::new(
        &[&payer],
        Message::new(&[create_ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );

    let res = svm.send_transaction(transaction);
    assert!(res.is_ok(), "Transaction failed: {:?}", res);

    // Verify On-Chain Data

    // Verify Vault
    let vault_account = svm.get_account(&vault_pda);
    match vault_account {
        Some(acc) => {
            assert_eq!(acc.data.len(), 0, "Vault should have 0 data");
            assert_eq!(
                acc.owner, system_program_id,
                "Vault owned by System Program"
            );
        },
        None => panic!("Vault account not found"),
    }

    // Verify Config
    let config_account = svm
        .get_account(&config_pda)
        .expect("Config account not found");
    assert_eq!(
        config_account.owner, program_id,
        "Config owner should be LazorKit program"
    );

    let data = config_account.data;
    assert!(data.len() >= LazorKitWallet::LEN);

    let disc = data[0];
    let stored_bump = data[1];
    // let stored_id = &data[2..34];
    let role_count = u16::from_le_bytes(data[34..36].try_into().unwrap());
    let role_counter = u32::from_le_bytes(data[36..40].try_into().unwrap());
    let stored_wallet_bump = data[40];

    assert_eq!(disc, 1, "Discriminator incorrect");
    assert_eq!(stored_bump, bump, "Bump incorrect");
    assert_eq!(role_count, 1, "Should have 1 role (owner)");
    assert_eq!(role_counter, 1, "Role counter should be 1");
    assert_eq!(stored_wallet_bump, wallet_bump, "Wallet bump incorrect");

    // Verify Owner Position
    let pos_start = LazorKitWallet::LEN;
    let pos_end = pos_start + Position::LEN;
    assert!(data.len() >= pos_end);

    let pos_data = &data[pos_start..pos_end];
    let auth_type_val = u16::from_le_bytes(pos_data[0..2].try_into().unwrap());
    let auth_len_val = u16::from_le_bytes(pos_data[2..4].try_into().unwrap());
    let num_actions = u16::from_le_bytes(pos_data[4..6].try_into().unwrap());
    let id_val = u32::from_le_bytes(pos_data[8..12].try_into().unwrap());
    let boundary = u32::from_le_bytes(pos_data[12..16].try_into().unwrap());

    assert_eq!(
        auth_type_val,
        AuthorityType::Ed25519 as u16,
        "Position auth type mismatch"
    );
    assert_eq!(
        auth_len_val as usize,
        auth_blob.len(),
        "Position auth len mismatch"
    );
    assert_eq!(num_actions, 0, "Initial plugins should be 0");
    assert_eq!(id_val, 0, "Owner Role ID must be 0");

    let expected_boundary = pos_len_check(pos_end, auth_blob.len());
    fn pos_len_check(start: usize, len: usize) -> usize {
        start + len
    }

    assert_eq!(
        boundary as usize, expected_boundary,
        "Boundary calculation error"
    );

    let stored_auth_data = &data[pos_end..pos_end + auth_blob.len()];
    assert_eq!(
        stored_auth_data,
        auth_blob.as_slice(),
        "Stored authority data mismatch"
    );
}

#[test]
fn test_create_wallet_with_secp256k1_authority() {
    let mut env = setup_env();
    let payer = env.payer;
    let mut svm = env.svm;
    let program_id = env.program_id;
    let program_id_pubkey = env.program_id;

    let wallet_id = [9u8; 32];
    let (config_pda, bump) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &program_id);
    let (vault_pda, wallet_bump) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &program_id,
    );

    // Create fake Secp256k1 key (64 bytes uncompressed)
    let fake_secp_key = [1u8; 64];

    // NOTE: LazorKit contract handles compression of 64 byte keys during initialization.
    // We pass 64 bytes, but it will be stored as 33 bytes (compressed) + padding/metadata.
    // However, CreateWallet instruction expects `owner_authority_data` to be passed in.
    // For Secp256k1, stored bytes are created via Authority trait's `set_into_bytes`.

    // Ideally we'd use `Secp256k1Authority` struct helper if available or construct raw bytes
    // But since we are passing `owner_authority_data` as vec, let's just pass the 64 bytes directly
    // and let the contract handle the conversion to its internal storage format.
    // WAIT: `process_create_wallet` calls `LazorKitBuilder::add_role`.
    // `add_role` calls `Authority::set_into_bytes` with the provided data.
    // For `Secp256k1Authority`, `set_into_bytes` takes 64 bytes and compresses it.

    let auth_blob = fake_secp_key.to_vec();

    let instruction = LazorKitInstruction::CreateWallet {
        id: wallet_id,
        bump,
        wallet_bump,
        owner_authority_type: AuthorityType::Secp256k1 as u16,
        owner_authority_data: auth_blob.clone(),
    };

    let instruction_data = borsh::to_vec(&instruction).unwrap();
    let system_program_id = system_program::id();

    let accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: system_program_id,
            is_signer: false,
            is_writable: false,
        },
    ];

    let create_ix = Instruction {
        program_id,
        accounts,
        data: instruction_data,
    };

    let transaction = Transaction::new(
        &[&payer],
        Message::new(&[create_ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );

    let res = svm.send_transaction(transaction);
    assert!(res.is_ok(), "Transaction failed: {:?}", res);

    // Verify storage
    let config_account = svm
        .get_account(&config_pda)
        .expect("Config account not found");
    let data = config_account.data;

    let pos_start = LazorKitWallet::LEN;
    let pos_len = Position::LEN;
    let pos_data = &data[pos_start..pos_start + pos_len];

    let auth_type_val = u16::from_le_bytes(pos_data[0..2].try_into().unwrap());
    let auth_len_val = u16::from_le_bytes(pos_data[2..4].try_into().unwrap());

    assert_eq!(
        auth_type_val,
        AuthorityType::Secp256k1 as u16,
        "Must be Secp256k1 type"
    );

    // Secp256k1Authority struct size: 33 bytes key + 3 bytes padding + 4 bytes odometer = 40 bytes.
    // Wait, let's allow the test to discover the size or hardcode expected size.
    // From secp256k1.rs:
    // pub struct Secp256k1Authority {
    //     pub public_key: [u8; 33],
    //     _padding: [u8; 3],  <-- aligns to 36
    //     pub signature_odometer: u32, <-- aligns to 40
    // }
    // It has `#[repr(C, align(8))]`. Size might be padded to 8 bytes multiple.
    // 40 is divisible by 8. So size should be 40.

    assert_eq!(
        auth_len_val, 40,
        "Secp256k1Authority storage size should be 40"
    );
}

#[test]
fn test_create_wallet_fail_invalid_seeds() {
    let mut env = setup_env();
    let payer = env.payer;
    let mut svm = env.svm;
    let program_id = env.program_id;
    let program_id_pubkey = env.program_id;

    let wallet_id = [8u8; 32];
    let (valid_config_pub, w_bump) =
        Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &program_id);
    let (valid_vault_pub, wallet_bump) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", valid_config_pub.as_ref()],
        &program_id,
    );

    let fake_config = Pubkey::new_unique();
    let fake_vault = Pubkey::new_unique();

    let authority_data = Ed25519Authority::new(payer.pubkey().to_bytes());
    use lazorkit_state::IntoBytes;
    let auth_blob = authority_data.into_bytes().unwrap().to_vec();

    let instruction = LazorKitInstruction::CreateWallet {
        id: wallet_id,
        bump: w_bump,
        wallet_bump,
        owner_authority_type: AuthorityType::Ed25519 as u16,
        owner_authority_data: auth_blob,
    };
    let instruction_data = borsh::to_vec(&instruction).unwrap();
    let system_program_id = system_program::id();

    // CASE 1: Wrong Config Account
    let accounts_bad_config = vec![
        AccountMeta {
            pubkey: fake_config,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: valid_vault_pub,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: system_program_id,
            is_signer: false,
            is_writable: false,
        },
    ];
    let ix_bad_config = Instruction {
        program_id,
        accounts: accounts_bad_config,
        data: instruction_data.clone(),
    };
    let tx_bad_config = Transaction::new(
        &[&payer],
        Message::new(&[ix_bad_config], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );

    let err = svm.send_transaction(tx_bad_config);
    assert!(err.is_err(), "Should verify seeds and fail on bad config");

    // CASE 2: Wrong Vault Account
    let accounts_bad_vault = vec![
        AccountMeta {
            pubkey: valid_config_pub,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: fake_vault,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: system_program_id,
            is_signer: false,
            is_writable: false,
        },
    ];
    let ix_bad_vault = Instruction {
        program_id,
        accounts: accounts_bad_vault,
        data: instruction_data,
    };
    let tx_bad_vault = Transaction::new(
        &[&payer],
        Message::new(&[ix_bad_vault], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );

    let err_vault = svm.send_transaction(tx_bad_vault);
    assert!(
        err_vault.is_err(),
        "Should verify seeds and fail on bad vault"
    );
}

#[test]
fn test_create_wallet_with_secp256r1_authority() {
    let mut env = setup_env();
    let payer = env.payer;
    let mut svm = env.svm;
    let program_id = env.program_id;
    let program_id_pubkey = env.program_id;

    let wallet_id = [5u8; 32];
    let (config_pda, bump) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &program_id);
    let (vault_pda, wallet_bump) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &program_id,
    );

    // Create fake Secp256r1 key (33 bytes compressed)
    // Note: Secp256r1 implementation requires strict 33-byte input
    let fake_secp_key = [2u8; 33];

    let auth_blob = fake_secp_key.to_vec();

    let instruction = LazorKitInstruction::CreateWallet {
        id: wallet_id,
        bump,
        wallet_bump,
        owner_authority_type: AuthorityType::Secp256r1 as u16,
        owner_authority_data: auth_blob.clone(),
    };

    let instruction_data = borsh::to_vec(&instruction).unwrap();
    let system_program_id = system_program::id();

    let accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: system_program_id,
            is_signer: false,
            is_writable: false,
        },
    ];

    let create_ix = Instruction {
        program_id,
        accounts,
        data: instruction_data,
    };

    let transaction = Transaction::new(
        &[&payer],
        Message::new(&[create_ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );

    let res = svm.send_transaction(transaction);
    assert!(res.is_ok(), "Transaction failed: {:?}", res);

    // Verify storage
    let config_account = svm
        .get_account(&config_pda)
        .expect("Config account not found");
    let data = config_account.data;

    let pos_start = LazorKitWallet::LEN;
    let pos_len = Position::LEN;
    let pos_data = &data[pos_start..pos_start + pos_len];

    let auth_type_val = u16::from_le_bytes(pos_data[0..2].try_into().unwrap());
    let auth_len_val = u16::from_le_bytes(pos_data[2..4].try_into().unwrap());

    assert_eq!(
        auth_type_val,
        AuthorityType::Secp256r1 as u16,
        "Must be Secp256r1 type"
    );

    // Secp256r1Authority struct size: 33 bytes public_key + 3 bytes padding + 4 bytes odometer = 40 bytes.
    assert_eq!(
        auth_len_val, 40,
        "Secp256r1Authority storage size should be 40"
    );
}

#[test]
fn test_create_wallet_fail_invalid_authority_type() {
    let mut env = setup_env();
    let payer = env.payer;
    let mut svm = env.svm;
    let program_id = env.program_id;
    let program_id_pubkey = env.program_id;

    let wallet_id = [12u8; 32];
    let (config_pda, bump) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &program_id);
    let (vault_pda, wallet_bump) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        &program_id,
    );
    let system_program_id = system_program::id();

    let accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: system_program_id,
            is_signer: false,
            is_writable: false,
        },
    ];

    let auth_blob = vec![0u8; 32];
    let instruction = LazorKitInstruction::CreateWallet {
        id: wallet_id,
        bump,
        wallet_bump,
        owner_authority_type: 999, // Invalid Type
        owner_authority_data: auth_blob,
    };
    let instruction_data = borsh::to_vec(&instruction).unwrap();

    let ix = Instruction {
        program_id,
        accounts,
        data: instruction_data,
    };
    let tx = Transaction::new(
        &[&payer],
        Message::new(&[ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );

    let res = svm.send_transaction(tx);
    // Should fail with InvalidInstructionData due to try_from failure
    assert!(res.is_err());
    let err = res.err().unwrap();
    // Verify it is InvalidInstructionData (code 0x0b or equivalent ProgramError)
    // litesvm returns TransactionError.
    println!("Invalid Type Error: {:?}", err);
}

#[test]
fn test_create_wallet_fail_invalid_authority_data_length() {
    let mut env = setup_env();
    let payer = env.payer;
    let mut svm = env.svm;
    let program_id = env.program_id;
    let program_id_pubkey = env.program_id;
    let system_program_id = system_program::id();

    // Subtest: Ed25519 invalid length (31 bytes instead of 32)
    {
        let wallet_id = [13u8; 32];
        let (config_pda, bump) =
            Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &program_id);
        let (vault_pda, wallet_bump) = Pubkey::find_program_address(
            &[b"lazorkit-wallet-address", config_pda.as_ref()],
            &program_id,
        );

        let accounts = vec![
            AccountMeta {
                pubkey: config_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: payer.pubkey(),
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: vault_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: system_program_id,
                is_signer: false,
                is_writable: false,
            },
        ];

        let auth_blob = vec![1u8; 31]; // Invalid for Ed25519
        let instruction = LazorKitInstruction::CreateWallet {
            id: wallet_id,
            bump,
            wallet_bump,
            owner_authority_type: AuthorityType::Ed25519 as u16,
            owner_authority_data: auth_blob,
        };
        let instruction_data = borsh::to_vec(&instruction).unwrap();
        let ix = Instruction {
            program_id,
            accounts,
            data: instruction_data,
        };
        let tx = Transaction::new(
            &[&payer],
            Message::new(&[ix], Some(&payer.pubkey())),
            svm.latest_blockhash(),
        );

        let res = svm.send_transaction(tx);
        assert!(res.is_err(), "Should fail Ed25519 with 31 bytes");
        // Expect LazorStateError::InvalidRoleData = 1002 + 2000 = 3002
        println!("Ed25519 Invalid Len Error: {:?}", res.err());
    }

    // Subtest: Secp256k1 invalid length (63 bytes)
    {
        let wallet_id = [14u8; 32];
        let (config_pda, bump) =
            Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &program_id);
        let (vault_pda, wallet_bump) = Pubkey::find_program_address(
            &[b"lazorkit-wallet-address", config_pda.as_ref()],
            &program_id,
        );

        let accounts = vec![
            AccountMeta {
                pubkey: config_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: payer.pubkey(),
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: vault_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: system_program_id,
                is_signer: false,
                is_writable: false,
            },
        ];

        let auth_blob = vec![1u8; 63]; // Invalid for Secp256k1 (needs 33 or 64)
        let instruction = LazorKitInstruction::CreateWallet {
            id: wallet_id,
            bump,
            wallet_bump,
            owner_authority_type: AuthorityType::Secp256k1 as u16,
            owner_authority_data: auth_blob,
        };
        let instruction_data = borsh::to_vec(&instruction).unwrap();
        let ix = Instruction {
            program_id,
            accounts,
            data: instruction_data,
        };
        let tx = Transaction::new(
            &[&payer],
            Message::new(&[ix], Some(&payer.pubkey())),
            svm.latest_blockhash(),
        );

        let res = svm.send_transaction(tx);
        assert!(res.is_err(), "Should fail Secp256k1 with 63 bytes");
        println!("Secp256k1 Invalid Len Error: {:?}", res.err());
    }
}
