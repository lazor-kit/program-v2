mod common;
use common::{bridge_tx, create_wallet, setup_env, to_sdk_hash, TestEnv};

use lazorkit_program::instruction::LazorKitInstruction;
use lazorkit_sol_limit_plugin::SolLimitState;
use lazorkit_state::{
    authority::{
        ed25519::Ed25519Authority, secp256k1::Secp256k1Authority, secp256r1::Secp256r1Authority,
        AuthorityType,
    },
    policy::PolicyHeader,
    registry::PolicyRegistryEntry,
    IntoBytes, LazorKitWallet, Position, Transmutable,
};
use solana_address::Address;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

// Helper to sign add authority instruction
#[derive(borsh::BorshSerialize)]
struct AddAuthPayload<'a> {
    acting_role_id: u32,
    authority_type: u16,
    authority_data: &'a [u8],
    policies_config: &'a [u8],
}

#[test]
fn test_add_authority_success_with_sol_limit_policy() {
    let mut env = setup_env();
    let wallet_id = [20u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, _) = create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);

    let new_auth_kp = Keypair::new();
    let new_auth_blob = Ed25519Authority::new(new_auth_kp.pubkey().to_bytes())
        .into_bytes()
        .unwrap()
        .to_vec();

    let limit_state = SolLimitState {
        amount: 5_000_000_000,
    };
    let boundary_offset = PolicyHeader::LEN + SolLimitState::LEN;
    let pinocchio_id = pinocchio::pubkey::Pubkey::from(env.sol_limit_id_pubkey.to_bytes());
    let policy_header = PolicyHeader::new(
        pinocchio_id,
        SolLimitState::LEN as u16,
        boundary_offset as u32,
    );

    let mut policies_config_bytes = Vec::new();
    policies_config_bytes.extend_from_slice(&policy_header.into_bytes().unwrap());
    policies_config_bytes.extend_from_slice(&limit_state.into_bytes().unwrap());

    // For Ed25519, authorization_data is [account_index_of_signer].
    // Payer (Owner) is at index 1 in add_accounts.
    let auth_data = vec![3u8];

    let add_instruction = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: new_auth_blob.clone(),
        policies_config: policies_config_bytes.clone(),
        authorization_data: auth_data,
    };

    // Register Policy
    let (registry_pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
        &[
            PolicyRegistryEntry::SEED_PREFIX,
            &env.sol_limit_id_pubkey.to_bytes(),
        ],
        &env.program_id,
    );
    let reg_ix = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta {
                pubkey: registry_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: env.payer.pubkey(),
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: env.system_program_id,
                is_signer: false,
                is_writable: false,
            },
        ],
        data: borsh::to_vec(&LazorKitInstruction::RegisterPolicy {
            policy_program_id: env.sol_limit_id_pubkey.to_bytes(),
        })
        .unwrap(),
    };
    env.svm
        .send_transaction(bridge_tx(Transaction::new(
            &[&env.payer],
            Message::new(&[reg_ix], Some(&env.payer.pubkey())),
            to_sdk_hash(env.svm.latest_blockhash()),
        )))
        .unwrap();

    let add_ix_data = borsh::to_vec(&add_instruction).unwrap();
    let add_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: owner_kp.pubkey(),
            is_signer: true,
            is_writable: false,
        },
        AccountMeta {
            pubkey: registry_pda,
            is_signer: false,
            is_writable: false,
        },
    ];

    let add_tx = Transaction::new(
        &[&env.payer, &owner_kp],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts: add_accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    env.svm.send_transaction(bridge_tx(add_tx)).unwrap();

    // Verify
    let config_account = env
        .svm
        .get_account(&Address::from(config_pda.to_bytes()))
        .expect("Config account not found");
    let data = config_account.data;
    let wallet_header_len = LazorKitWallet::LEN;
    let wallet_data = &data[0..wallet_header_len];
    let role_count = u16::from_le_bytes(wallet_data[34..36].try_into().unwrap());
    assert_eq!(role_count, 2);

    let role0_pos_data = &data[wallet_header_len..wallet_header_len + Position::LEN];
    let role0_pos = unsafe { Position::load_unchecked(role0_pos_data).unwrap() };

    let role1_offset = role0_pos.boundary as usize;
    let role1_pos_data = &data[role1_offset..role1_offset + Position::LEN];
    let role1_pos = unsafe { Position::load_unchecked(role1_pos_data).unwrap() };

    assert_eq!(role1_pos.id, 1);
    assert_eq!(role1_pos.authority_type, AuthorityType::Ed25519 as u16);
    assert_eq!(role1_pos.num_policies, 1);

    // Verify Policy Data
    let action_offset = role1_offset + Position::LEN + role1_pos.authority_length as usize;
    let header_slice = &data[action_offset..action_offset + PolicyHeader::LEN];
    let stored_header = unsafe { PolicyHeader::load_unchecked(header_slice).unwrap() };

    assert_eq!(stored_header.program_id, pinocchio_id);
    assert_eq!(stored_header.data_length, SolLimitState::LEN as u16);

    let state_slice = &data
        [action_offset + PolicyHeader::LEN..action_offset + PolicyHeader::LEN + SolLimitState::LEN];
    let stored_state = unsafe { SolLimitState::load_unchecked(state_slice).unwrap() };
    assert_eq!(stored_state.amount, 5_000_000_000);
}

#[test]
fn test_add_authority_success_ed25519_no_policies() {
    let mut env = setup_env();
    let wallet_id = [21u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, _) = create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);

    let new_auth_kp = Keypair::new();
    let new_auth_blob = Ed25519Authority::new(new_auth_kp.pubkey().to_bytes())
        .into_bytes()
        .unwrap()
        .to_vec();
    let policies_config_bytes: Vec<u8> = Vec::new();

    let add_instruction = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: new_auth_blob.clone(),
        policies_config: policies_config_bytes.clone(),
        authorization_data: vec![3u8],
    };

    let add_ix_data = borsh::to_vec(&add_instruction).unwrap();
    let add_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: owner_kp.pubkey(),
            is_signer: true,
            is_writable: false,
        },
    ];
    let add_tx = Transaction::new(
        &[&env.payer, &owner_kp],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts: add_accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    env.svm.send_transaction(bridge_tx(add_tx)).unwrap();

    let config_account = env
        .svm
        .get_account(&Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = config_account.data;
    let role0_offset = LazorKitWallet::LEN;
    let role0_pos = unsafe {
        Position::load_unchecked(&data[role0_offset..role0_offset + Position::LEN]).unwrap()
    };
    let role1_offset = role0_pos.boundary as usize;
    let role1_pos = unsafe {
        Position::load_unchecked(&data[role1_offset..role1_offset + Position::LEN]).unwrap()
    };
    assert_eq!(role1_pos.id, 1);
    assert_eq!(role1_pos.num_policies, 0);
}

#[test]
fn test_add_authority_success_secp256k1_with_policy() {
    let mut env = setup_env();
    let wallet_id = [22u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, _) = create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);

    let secp_key = [7u8; 33];
    let new_auth_blob = Secp256k1Authority::new(secp_key)
        .into_bytes()
        .unwrap()
        .to_vec();

    let limit_state = SolLimitState { amount: 1_000_000 };
    let boundary_offset = PolicyHeader::LEN + SolLimitState::LEN;
    let pinocchio_id = pinocchio::pubkey::Pubkey::from(env.sol_limit_id_pubkey.to_bytes());
    let policy_header = PolicyHeader::new(
        pinocchio_id,
        SolLimitState::LEN as u16,
        boundary_offset as u32,
    );
    let mut policies_config_bytes = Vec::new();
    policies_config_bytes.extend_from_slice(&policy_header.into_bytes().unwrap());
    policies_config_bytes.extend_from_slice(&limit_state.into_bytes().unwrap());

    let add_instruction = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Secp256k1 as u16,
        authority_data: new_auth_blob.clone(),
        policies_config: policies_config_bytes.clone(),
        authorization_data: vec![3u8],
    };

    // Register Policy
    let (registry_pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
        &[
            PolicyRegistryEntry::SEED_PREFIX,
            &env.sol_limit_id_pubkey.to_bytes(),
        ],
        &env.program_id,
    );
    let reg_ix = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta {
                pubkey: registry_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: env.payer.pubkey(),
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: env.system_program_id,
                is_signer: false,
                is_writable: false,
            },
        ],
        data: borsh::to_vec(&LazorKitInstruction::RegisterPolicy {
            policy_program_id: env.sol_limit_id_pubkey.to_bytes(),
        })
        .unwrap(),
    };
    env.svm
        .send_transaction(bridge_tx(Transaction::new(
            &[&env.payer],
            Message::new(&[reg_ix], Some(&env.payer.pubkey())),
            to_sdk_hash(env.svm.latest_blockhash()),
        )))
        .unwrap();

    let add_ix_data = borsh::to_vec(&add_instruction).unwrap();
    let add_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: owner_kp.pubkey(),
            is_signer: true,
            is_writable: false,
        },
        AccountMeta {
            pubkey: registry_pda,
            is_signer: false,
            is_writable: false,
        },
    ];
    let add_tx = Transaction::new(
        &[&env.payer, &owner_kp],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts: add_accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    env.svm.send_transaction(bridge_tx(add_tx)).unwrap();

    let config_account = env
        .svm
        .get_account(&Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = config_account.data;
    let role0_offset = LazorKitWallet::LEN;
    let role0_pos = unsafe {
        Position::load_unchecked(&data[role0_offset..role0_offset + Position::LEN]).unwrap()
    };
    let role1_offset = role0_pos.boundary as usize;
    let role1_pos = unsafe {
        Position::load_unchecked(&data[role1_offset..role1_offset + Position::LEN]).unwrap()
    };
    assert_eq!(role1_pos.id, 1);
    assert_eq!(role1_pos.authority_type, AuthorityType::Secp256k1 as u16);
    assert_eq!(role1_pos.authority_length, 40);

    let action_offset = role1_offset + Position::LEN + role1_pos.authority_length as usize;
    let header_slice = &data[action_offset..action_offset + PolicyHeader::LEN];
    let stored_header = unsafe { PolicyHeader::load_unchecked(header_slice).unwrap() };
    assert_eq!(stored_header.program_id, pinocchio_id);

    let state_slice = &data
        [action_offset + PolicyHeader::LEN..action_offset + PolicyHeader::LEN + SolLimitState::LEN];
    let stored_state = unsafe { SolLimitState::load_unchecked(state_slice).unwrap() };
    assert_eq!(stored_state.amount, 1_000_000);
}

#[test]
fn test_add_authority_fail_unauthorized_signer() {
    let mut env = setup_env();
    let wallet_id = [23u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, _) = create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);

    let new_auth_kp = Keypair::new();
    let new_auth_blob = Ed25519Authority::new(new_auth_kp.pubkey().to_bytes())
        .into_bytes()
        .unwrap()
        .to_vec();

    // Use a different key to sign (Unauthorized)
    let other_kp = Keypair::new();

    // Note: acting_role_id is 0 (Owner), but we sign with someone else.
    // Auth check should fail because stored Owner Key != other_kp pubkey.

    let add_instruction = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: new_auth_blob,
        policies_config: vec![],
        authorization_data: vec![3u8],
    };

    let add_ix_data = borsh::to_vec(&add_instruction).unwrap();
    let add_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: other_kp.pubkey(),
            is_signer: true,
            is_writable: false,
        }, // other_kp signs
    ];
    let add_tx = Transaction::new(
        &[&env.payer, &other_kp],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts: add_accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    let res = env.svm.send_transaction(bridge_tx(add_tx));
    assert!(res.is_err());
    // Should be Program Error for Invalid Signature or Unauthorized depending on impl details.
    // Authenticate usually returns ProgramError.
}

#[test]
fn test_add_authority_fail_invalid_authority_type() {
    let mut env = setup_env();
    let wallet_id = [24u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, _) = create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let new_auth_blob = vec![0u8; 32];

    let add_instruction = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: 9999,
        authority_data: new_auth_blob,
        policies_config: vec![],
        authorization_data: vec![3u8],
    };
    // ... verification logic ...
    let add_ix_data = borsh::to_vec(&add_instruction).unwrap();
    let add_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: config_pda, // Using config twice as placeholder or any writable? Payer is next.
            is_signer: false,
            is_writable: true,
        }, // Wait, previous test structure was consistent. Let's stick to standard 4 accounts.
           // accounts = [config, payer, system, auth_signer]
    ];

    // Correction: Standard AddAuthority accounts are:
    // 0: Config (W)
    // 1: Payer (S, W)
    // 2: System (R)
    // 3: Owner/Signer (S)
    let add_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: owner_kp.pubkey(),
            is_signer: true,
            is_writable: false,
        },
    ];

    let add_tx = Transaction::new(
        &[&env.payer, &owner_kp],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts: add_accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    let res = env.svm.send_transaction(bridge_tx(add_tx));
    assert!(res.is_err());
}

#[test]
fn test_add_authority_fail_invalid_authority_length() {
    let mut env = setup_env();
    let wallet_id = [25u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, _) = create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let invalid_auth_blob = vec![0u8; 31];

    let add_instruction = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: invalid_auth_blob,
        policies_config: vec![],
        authorization_data: vec![3u8],
    };
    // ... verification logic ...
    let add_ix_data = borsh::to_vec(&add_instruction).unwrap();
    let add_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: owner_kp.pubkey(),
            is_signer: true,
            is_writable: false,
        },
    ];
    let add_tx = Transaction::new(
        &[&env.payer, &owner_kp],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts: add_accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    let res = env.svm.send_transaction(bridge_tx(add_tx));
    assert!(res.is_err());
}

#[test]
fn test_add_authority_success_secp256r1_with_policy() {
    let mut env = setup_env();
    let wallet_id = [26u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, _) = create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);

    let mut secp_key = [0u8; 33];
    secp_key[0] = 0x02;
    secp_key[1] = 0xAA;

    let new_auth_blob = Secp256r1Authority::new(secp_key)
        .into_bytes()
        .unwrap()
        .to_vec();

    let limit_state = SolLimitState { amount: 2_000_000 };
    let boundary_offset = PolicyHeader::LEN + SolLimitState::LEN;
    let pinocchio_id = pinocchio::pubkey::Pubkey::from(env.sol_limit_id_pubkey.to_bytes());
    let policy_header = PolicyHeader::new(
        pinocchio_id,
        SolLimitState::LEN as u16,
        boundary_offset as u32,
    );
    let mut policies_config_bytes = Vec::new();
    policies_config_bytes.extend_from_slice(&policy_header.into_bytes().unwrap());
    policies_config_bytes.extend_from_slice(&limit_state.into_bytes().unwrap());

    let add_instruction = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Secp256r1 as u16,
        authority_data: new_auth_blob.clone(),
        policies_config: policies_config_bytes.clone(),
        authorization_data: vec![3u8],
    };

    // Register Policy
    let (registry_pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
        &[
            PolicyRegistryEntry::SEED_PREFIX,
            &env.sol_limit_id_pubkey.to_bytes(),
        ],
        &env.program_id,
    );
    let reg_ix = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta {
                pubkey: registry_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: env.payer.pubkey(),
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: env.system_program_id,
                is_signer: false,
                is_writable: false,
            },
        ],
        data: borsh::to_vec(&LazorKitInstruction::RegisterPolicy {
            policy_program_id: env.sol_limit_id_pubkey.to_bytes(),
        })
        .unwrap(),
    };
    env.svm
        .send_transaction(bridge_tx(Transaction::new(
            &[&env.payer],
            Message::new(&[reg_ix], Some(&env.payer.pubkey())),
            to_sdk_hash(env.svm.latest_blockhash()),
        )))
        .unwrap();

    let add_ix_data = borsh::to_vec(&add_instruction).unwrap();
    let add_accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: owner_kp.pubkey(),
            is_signer: true,
            is_writable: false,
        },
        AccountMeta {
            pubkey: registry_pda,
            is_signer: false,
            is_writable: false,
        },
    ];
    let add_tx = Transaction::new(
        &[&env.payer, &owner_kp],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts: add_accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    env.svm.send_transaction(bridge_tx(add_tx)).unwrap();

    let config_account = env
        .svm
        .get_account(&Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = config_account.data;
    let role0_offset = LazorKitWallet::LEN;
    let role0_pos = unsafe {
        Position::load_unchecked(&data[role0_offset..role0_offset + Position::LEN]).unwrap()
    };
    let role1_offset = role0_pos.boundary as usize;
    let role1_pos = unsafe {
        Position::load_unchecked(&data[role1_offset..role1_offset + Position::LEN]).unwrap()
    };
    assert_eq!(role1_pos.id, 1);
    assert_eq!(role1_pos.authority_type, AuthorityType::Secp256r1 as u16);
    assert_eq!(role1_pos.authority_length, 40);
}
