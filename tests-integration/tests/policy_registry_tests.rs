use lazorkit_program::instruction::LazorKitInstruction;
use lazorkit_state::{
    authority::{ed25519::Ed25519Authority, AuthorityType},
    registry::PolicyRegistryEntry,
    IntoBytes,
};
use pinocchio::pubkey::Pubkey as PinocchioPubkey;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod common;
use common::*;

#[test]
fn test_register_policy_happy_path() {
    let mut env = setup_env();
    let policy_id = Keypair::new().pubkey();
    let policy_id_bytes = policy_id.to_bytes();

    let (registry_pda, _bump) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id_bytes],
        &env.program_id,
    );

    let ix_data = borsh::to_vec(&LazorKitInstruction::RegisterPolicy {
        policy_program_id: policy_id_bytes,
    })
    .unwrap();

    let accounts = vec![
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
    ];

    let ix = Instruction {
        program_id: env.program_id,
        accounts,
        data: ix_data,
    };

    let tx = Transaction::new(
        &[&env.payer],
        Message::new(&[ix], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    let v_tx = bridge_tx(tx);
    let res = env.svm.send_transaction(v_tx);
    assert!(res.is_ok());

    // Verify registry account exists and data is correct
    let acc = env
        .svm
        .get_account(&solana_address::Address::from(registry_pda.to_bytes()));
    assert!(acc.is_some());
    let data = acc.unwrap().data;
    assert_eq!(data.len(), PolicyRegistryEntry::LEN);
    // Offset 16..48 is policy_program_id
    assert_eq!(&data[16..48], &policy_id_bytes);
    // Offset 48 is is_active (1)
    assert_eq!(data[48], 1);
}

#[test]
fn test_deactivate_policy() {
    let mut env = setup_env();
    let policy_id = Keypair::new().pubkey();
    let policy_id_bytes = policy_id.to_bytes();

    let (registry_pda, _bump) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id_bytes],
        &env.program_id,
    );

    // 1. Register Policy
    let register_ix_data = borsh::to_vec(&LazorKitInstruction::RegisterPolicy {
        policy_program_id: policy_id_bytes,
    })
    .unwrap();

    let register_accounts = vec![
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
    ];

    let register_ix = Instruction {
        program_id: env.program_id,
        accounts: register_accounts.clone(),
        data: register_ix_data,
    };

    let tx = Transaction::new(
        &[&env.payer],
        Message::new(&[register_ix], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    env.svm.send_transaction(bridge_tx(tx)).unwrap();

    // 2. Deactivate Policy
    let deactivate_ix_data = borsh::to_vec(&LazorKitInstruction::DeactivatePolicy {
        policy_program_id: policy_id_bytes,
    })
    .unwrap();

    // Deactivate requires Admin signer (env.payer is Admin for now as per handler check)
    let deactivate_accounts = vec![
        AccountMeta {
            pubkey: registry_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(), // Admin
            is_signer: true,
            is_writable: true,
        },
    ];

    let deactivate_ix = Instruction {
        program_id: env.program_id,
        accounts: deactivate_accounts,
        data: deactivate_ix_data,
    };

    let tx = Transaction::new(
        &[&env.payer],
        Message::new(&[deactivate_ix], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    let res = env.svm.send_transaction(bridge_tx(tx));
    assert!(res.is_ok());

    // Verify is_active is 0
    let acc = env
        .svm
        .get_account(&solana_address::Address::from(registry_pda.to_bytes()))
        .unwrap();
    assert_eq!(acc.data[48], 0);
}

#[test]
fn test_add_authority_unverified_policy_fails() {
    let mut env = setup_env();
    // Wallet creation moved to below to use payer as owner
    let wallet_id = [1u8; 32];

    let policy_id = Keypair::new().pubkey();
    let policy_id_bytes = policy_id.to_bytes();

    // Calculate Registry PDA but DO NOT register it
    let (registry_pda, _) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id_bytes],
        &env.program_id,
    );

    let auth_data = Ed25519Authority::new(env.payer.pubkey().to_bytes())
        .into_bytes()
        .unwrap()
        .to_vec();

    // Fix borrow checker: clone payer keypair
    let owner_keypair = Keypair::from_bytes(&env.payer.to_bytes()).unwrap();
    let (config_pda, _) =
        create_wallet(&mut env, wallet_id, &owner_keypair, AuthorityType::Ed25519); // Payer is Owner

    use lazorkit_state::policy::PolicyHeader;

    let header = PolicyHeader::new(
        PinocchioPubkey::from(policy_id_bytes),
        0,
        PolicyHeader::LEN as u32,
    );

    let policies_config = vec![header.into_bytes().unwrap()].concat();

    // Authorization: vec![1] means account at index 1 (Payer) is the signer authorizing this.
    // Index 0: Config
    // Index 1: Payer
    let authorization_data = vec![1];

    let add_ix_data = borsh::to_vec(&LazorKitInstruction::AddAuthority {
        acting_role_id: 0, // Owner
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: auth_data.clone(), // Adding same auth just to test policy check
        policies_config,
        authorization_data,
    })
    .unwrap();

    let accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(), // Signer (Owner)
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        // Remaining Accounts: Registry PDA
        AccountMeta {
            pubkey: registry_pda,
            is_signer: false,
            is_writable: false,
        },
    ];

    let add_ix = Instruction {
        program_id: env.program_id,
        accounts,
        data: add_ix_data,
    };

    let tx = Transaction::new(
        &[&env.payer],
        Message::new(&[add_ix], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    let res = env.svm.send_transaction(bridge_tx(tx));
    assert!(res.is_err());

    // Check error code 11 (UnverifiedPolicy)
}

#[test]
fn test_add_authority_deactivated_policy_fails() {
    let mut env = setup_env();
    let wallet_id = [3u8; 32];
    let owner_keypair = Keypair::from_bytes(&env.payer.to_bytes()).unwrap();
    let (config_pda, _) =
        create_wallet(&mut env, wallet_id, &owner_keypair, AuthorityType::Ed25519);

    let policy_id = Keypair::new().pubkey();
    let policy_id_bytes = policy_id.to_bytes();

    let (registry_pda, _) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id_bytes],
        &env.program_id,
    );

    // 1. Register
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
            policy_program_id: policy_id_bytes,
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

    // 2. Deactivate
    let deact_ix = Instruction {
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
        ],
        data: borsh::to_vec(&LazorKitInstruction::DeactivatePolicy {
            policy_program_id: policy_id_bytes,
        })
        .unwrap(),
    };
    env.svm
        .send_transaction(bridge_tx(Transaction::new(
            &[&env.payer],
            Message::new(&[deact_ix], Some(&env.payer.pubkey())),
            to_sdk_hash(env.svm.latest_blockhash()),
        )))
        .unwrap();

    // 3. Add Authority with Deactivated Policy
    use lazorkit_state::policy::PolicyHeader;
    let header = PolicyHeader::new(
        PinocchioPubkey::from(policy_id_bytes),
        0,
        PolicyHeader::LEN as u32,
    );
    let policies_config = vec![header.into_bytes().unwrap()].concat();
    let authorization_data = vec![1];
    let auth_data = Ed25519Authority::new(env.payer.pubkey().to_bytes())
        .into_bytes()
        .unwrap()
        .to_vec(); // Dummy new auth

    let add_ix_data = borsh::to_vec(&LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: auth_data,
        policies_config,
        authorization_data,
    })
    .unwrap();

    let accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.payer.pubkey(), // Signer (Owner)
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        },
        AccountMeta {
            pubkey: registry_pda,
            is_signer: false,
            is_writable: false,
        },
    ];

    let tx = Transaction::new(
        &[&env.payer],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    let res = env.svm.send_transaction(bridge_tx(tx));
    assert!(res.is_err());
    // Should fail with PolicyDeactivated (12)
}

#[test]
fn test_add_authority_verified_policy_success() {
    let mut env = setup_env();
    let wallet_id = [4u8; 32];
    let owner_keypair = Keypair::from_bytes(&env.payer.to_bytes()).unwrap();
    let (config_pda, _) =
        create_wallet(&mut env, wallet_id, &owner_keypair, AuthorityType::Ed25519);

    let policy_id = Keypair::new().pubkey();
    let policy_id_bytes = policy_id.to_bytes();

    let (registry_pda, _) = Pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, &policy_id_bytes],
        &env.program_id,
    );

    // 1. Register
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
            policy_program_id: policy_id_bytes,
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

    // 2. Add Authority with Verified Policy
    use lazorkit_state::policy::PolicyHeader;
    let header = PolicyHeader::new(
        PinocchioPubkey::from(policy_id_bytes),
        0,
        PolicyHeader::LEN as u32,
    );
    let policies_config = vec![header.into_bytes().unwrap()].concat();
    let authorization_data = vec![1];
    let new_auth_key = Keypair::new();
    let auth_data = Ed25519Authority::new(new_auth_key.pubkey().to_bytes())
        .into_bytes()
        .unwrap()
        .to_vec();

    let add_ix_data = borsh::to_vec(&LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: auth_data,
        policies_config,
        authorization_data,
    })
    .unwrap();

    let accounts = vec![
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
            pubkey: registry_pda,
            is_signer: false,
            is_writable: false,
        },
    ];

    let tx = Transaction::new(
        &[&env.payer],
        Message::new(
            &[Instruction {
                program_id: env.program_id,
                accounts,
                data: add_ix_data,
            }],
            Some(&env.payer.pubkey()),
        ),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    let res = env.svm.send_transaction(bridge_tx(tx));
    assert!(res.is_ok());

    // Verify state
    let wallet_acc = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    // Role count should be 2. Offset 34 is role_count (u16)
    let role_count = u16::from_le_bytes(wallet_acc.data[34..36].try_into().unwrap());
    assert_eq!(role_count, 2);
}
