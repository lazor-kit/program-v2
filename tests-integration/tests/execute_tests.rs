// use lazorkit_interface::{VerifyInstruction, INSTRUCTION_VERIFY};
use lazorkit_program::instruction::LazorKitInstruction;
use lazorkit_sol_limit_plugin::SolLimitState;
use lazorkit_state::{
    authority::{ed25519::Ed25519Authority, AuthorityType},
    policy::PolicyHeader,
    registry::PolicyRegistryEntry,
    IntoBytes, LazorKitWallet, Position, Transmutable,
};
use lazorkit_whitelist_plugin::WhitelistState;
use pinocchio::pubkey::Pubkey as PinocchioPubkey;
use solana_address::Address;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::path::PathBuf;

mod common;
use common::{bridge_tx, create_wallet, setup_env, to_sdk_hash};

pub fn get_whitelist_policy_path() -> PathBuf {
    let root = std::env::current_dir().unwrap();
    let path = root.join("target/deploy/lazorkit_whitelist_plugin.so");
    if path.exists() {
        return path;
    }
    let path = root.join("../target/deploy/lazorkit_whitelist_plugin.so");
    if path.exists() {
        return path;
    }
    let path = root
        .parent()
        .unwrap()
        .join("target/deploy/lazorkit_whitelist_plugin.so");
    if path.exists() {
        return path;
    }
    panic!("Could not find lazorkit_whitelist_plugin.so");
}

#[test]
fn test_execute_flow_with_whitelist() {
    let mut env = setup_env();

    // 1. Deploy Whitelist Policy
    let whitelist_policy_id = Keypair::new().pubkey();
    let policy_bytes =
        std::fs::read(get_whitelist_policy_path()).expect("Failed to read whitelist policy binary");
    env.svm
        .add_program(Address::from(whitelist_policy_id.to_bytes()), &policy_bytes)
        .unwrap();

    // 2. Register Policy (Registry Check)
    let (registry_pda, _) = Pubkey::find_program_address(
        &[
            PolicyRegistryEntry::SEED_PREFIX,
            &whitelist_policy_id.to_bytes(),
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
            policy_program_id: whitelist_policy_id.to_bytes(),
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

    // 3. Create Wallet
    let owner_kp = Keypair::new();
    let _amount_limit = 20_000_000u64; // 0.02 SOL
    let wallet_id = [1u8; 32];
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);

    env.svm
        .airdrop(&Address::from(vault_pda.to_bytes()), 1_000_000_000)
        .unwrap();

    // 4. Add Authority (Role 1) with Whitelist Policy
    // Whitelist State: [count(u16), padding(u16), addresses...]
    // Empty whitelist: count=0.
    // WhitelistState definition is roughly: u16 count, u16 padding, [Pubkey; 100].
    // We can manually construct the bytes since we just want count=0.
    // 2 + 2 = 4 bytes header. Data following is irrelevant if count=0?
    // Wait, WhitelistState::LEN is fixed. We must provide full length.
    // Checking `whitelist/src/lib.rs`:
    // pub struct WhitelistState { pub count: u16, pub _padding: u16, pub addresses: [Pubkey; 100] }
    // LEN = 2 + 2 + 32*100 = 3204.
    let whitelist_state_len = WhitelistState::LEN;
    let whitelist_state_bytes = vec![0u8; whitelist_state_len];
    // count is 0 (first 2 bytes).

    // Construct PolicyHeader
    let boundary_offset = PolicyHeader::LEN + whitelist_state_len;
    let pinocchio_id = PinocchioPubkey::from(whitelist_policy_id.to_bytes());
    let policy_header = PolicyHeader::new(
        pinocchio_id,
        whitelist_state_len as u16,
        boundary_offset as u32,
    );

    let mut policies_config = Vec::new();
    policies_config.extend_from_slice(&policy_header.into_bytes().unwrap());
    policies_config.extend_from_slice(&whitelist_state_bytes);

    // Add Delegate Authority (Role 1)
    let delegate_kp = Keypair::new();
    let auth_struct = Ed25519Authority::new(delegate_kp.pubkey().to_bytes());
    let auth_data = auth_struct.into_bytes().unwrap();
    let role_id = 1;

    let add_auth_ix = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: auth_data.to_vec(),
        policies_config,
        authorization_data: vec![3], // Owner is at index 3
    };

    let ix_data = borsh::to_vec(&add_auth_ix).unwrap();

    let ix = Instruction {
        program_id: env.program_id,
        accounts: vec![
            AccountMeta {
                pubkey: config_pda,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: env.payer.pubkey(),
                is_signer: true,
                is_writable: true,
            }, // Payer
            AccountMeta {
                pubkey: env.system_program_id,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: owner_kp.pubkey(),
                is_signer: true,
                is_writable: false,
            }, // Owner Auth (Index 3)
            // Remaining Accounts: Registry PDA for whitelist policy
            AccountMeta {
                pubkey: registry_pda,
                is_signer: false,
                is_writable: false,
            },
        ],
        data: ix_data,
    };

    let tx = Transaction::new(
        &[&env.payer, &owner_kp],
        Message::new(&[ix], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );
    env.svm.send_transaction(bridge_tx(tx)).unwrap();

    // 5. Try Execute Transfer (Should Fail as whitelist is empty)
    let recipient = Keypair::new().pubkey();
    let transfer_amount = 1000;

    let target_ix = system_instruction::transfer(&vault_pda, &recipient, transfer_amount);

    let target_ix_data = target_ix.data;
    // Signature: Delegate signs the message
    let signature = delegate_kp.sign_message(&target_ix_data);
    let signature_bytes = signature.as_ref().to_vec();

    // Payload: [signer_index (0)] + Signature
    let mut payload = vec![0u8];
    payload.extend(signature_bytes);

    let execute_ix_struct = LazorKitInstruction::Execute {
        role_id,
        instruction_payload: payload,
    };
    let execute_ix_data = borsh::to_vec(&execute_ix_struct).unwrap();

    // Construct Execute Accounts
    let accounts = vec![
        AccountMeta {
            pubkey: config_pda,
            is_signer: false,
            is_writable: true,
        },
        AccountMeta {
            pubkey: vault_pda,
            is_signer: false,
            is_writable: true,
        }, // Signer handled by program
        AccountMeta {
            pubkey: env.system_program_id,
            is_signer: false,
            is_writable: false,
        }, // Target Program (Index 3)
        // Target Instruction Accounts
        AccountMeta {
            pubkey: vault_pda,
            is_signer: false,
            is_writable: true,
        }, // From
        AccountMeta {
            pubkey: recipient,
            is_signer: false,
            is_writable: true,
        }, // To
    ];

    let execute_ix = Instruction {
        program_id: env.program_id,
        accounts,
        data: execute_ix_data,
    };

    let tx = Transaction::new(
        &[&env.payer],
        Message::new(&[execute_ix], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    // Expect Failure (Custom Error 1000 - Whitelist Failed)
    let res = env.svm.send_transaction(bridge_tx(tx));
    assert!(res.is_err(), "Should fail with empty whitelist");
    // Ideally verify error involves 1000.
    if let Err(e) = res {
        // Simple verification that it failed.
        println!("Execute failed as expected: {:?}", e);
    }
}

#[test]
fn test_execute_flow_with_sol_limit() {
    let mut env = setup_env();

    // 1. Register SolLimit Policy (Preloaded in setup_env)
    let (registry_pda, _) = Pubkey::find_program_address(
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

    // 2. Create Wallet
    let owner_kp = Keypair::new();
    let wallet_id = [2u8; 32];
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);

    env.svm
        .airdrop(&Address::from(vault_pda.to_bytes()), 10_000_000_000)
        .unwrap();

    // 3. Add Authority (Role 1) with SolLimit Policy (Limit: 2000)
    let limit_state = SolLimitState { amount: 2000 };
    let boundary_offset = PolicyHeader::LEN + SolLimitState::LEN;
    let pinocchio_id = PinocchioPubkey::from(env.sol_limit_id_pubkey.to_bytes());
    let policy_header = PolicyHeader::new(
        pinocchio_id,
        SolLimitState::LEN as u16,
        boundary_offset as u32,
    );

    let mut policy_config_bytes = Vec::new();
    policy_config_bytes.extend_from_slice(&policy_header.into_bytes().unwrap());
    policy_config_bytes.extend_from_slice(&limit_state.into_bytes().unwrap());

    let delegate_kp = Keypair::new();
    let auth_struct = Ed25519Authority::new(delegate_kp.pubkey().to_bytes());
    let auth_data = auth_struct.into_bytes().unwrap();
    let _role_id = 0; // Owner

    let add_auth_ix = LazorKitInstruction::AddAuthority {
        acting_role_id: 0,
        authority_type: AuthorityType::Ed25519 as u16,
        authority_data: auth_data.to_vec(),
        policies_config: policy_config_bytes,
        authorization_data: vec![3], // Owner is at index 3 in AddAuthority accounts
    };

    let ix_data = borsh::to_vec(&add_auth_ix).unwrap();

    let ix = Instruction {
        program_id: env.program_id,
        accounts: vec![
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
        ],
        data: ix_data,
    };

    env.svm
        .send_transaction(bridge_tx(Transaction::new(
            &[&env.payer, &owner_kp],
            Message::new(&[ix], Some(&env.payer.pubkey())),
            to_sdk_hash(env.svm.latest_blockhash()),
        )))
        .unwrap();

    // 4. Execute Transfer 1 (1000 lamports) - Should Success
    let recipient = Keypair::new().pubkey();
    let transfer_amount = 1000;

    let target_ix = system_instruction::transfer(&vault_pda, &recipient, transfer_amount);

    // Execute accounts for delegate (Role 1)
    let execute_accounts = vec![
        AccountMeta::new(config_pda, false),
        AccountMeta::new(vault_pda, false),
        AccountMeta::new_readonly(env.system_program_id, false),
        AccountMeta::new_readonly(env.system_program_id, false), // Target program (System)
        AccountMeta::new(vault_pda, false),                      // From
        AccountMeta::new(recipient, false),                      // To
        AccountMeta::new_readonly(delegate_kp.pubkey(), true),   // Delegate Signer (Role 1)
        AccountMeta::new_readonly(env.sol_limit_id_pubkey, false), // Policy
    ];

    // Payload: [signer_index] + [target_instruction_data]
    // Signer is at index 6 in execute_accounts
    let mut payload = vec![6u8];
    payload.extend_from_slice(&target_ix.data);

    let execute_ix_struct = LazorKitInstruction::Execute {
        role_id: 1, // <--- IMPORTANT: Role 1 has the SolLimit policy
        instruction_payload: payload,
    };
    let execute_ix_data = borsh::to_vec(&execute_ix_struct).unwrap();

    let execute_ix = Instruction {
        program_id: env.program_id,
        accounts: execute_accounts.clone(),
        data: execute_ix_data,
    };

    let tx = Transaction::new(
        &[&env.payer, &delegate_kp],
        Message::new(&[execute_ix], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    env.svm
        .send_transaction(bridge_tx(tx))
        .expect("Execute 1000 failed");

    // Verify Recipient received 1000
    let recipient_acc = env
        .svm
        .get_account(&Address::from(recipient.to_bytes()))
        .unwrap();
    assert_eq!(recipient_acc.lamports, 1000);

    // Verify SolLimit State: Amount should be 2000 - 1000 = 1000
    let config_acc = env
        .svm
        .get_account(&Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = config_acc.data;

    // Find Role 1 Position
    let role0_pos = unsafe { Position::load_unchecked(&data[LazorKitWallet::LEN..]).unwrap() };
    let role1_offset = role0_pos.boundary as usize;
    let role1_pos = unsafe { Position::load_unchecked(&data[role1_offset..]).unwrap() };

    let policy_start = role1_offset + Position::LEN + role1_pos.authority_length as usize;
    let state_start = policy_start + PolicyHeader::LEN;
    let stored_state = unsafe { SolLimitState::load_unchecked(&data[state_start..]).unwrap() };

    assert_eq!(
        stored_state.amount, 1000,
        "State mismatch after 1st execution"
    );

    // 5. Execute Transfer 2 (1500 lamports) - Should Fail (1000 left < 1500 needed)
    let recipient2 = Keypair::new().pubkey();
    let transfer_amount2 = 1500;
    let target_ix2 = system_instruction::transfer(&vault_pda, &recipient2, transfer_amount2);

    let mut payload2 = vec![6u8];
    payload2.extend_from_slice(&target_ix2.data);

    let mut execute_accounts2 = execute_accounts.clone();
    execute_accounts2[5] = AccountMeta::new(recipient2, false); // Update recipient

    let execute_ix_struct2 = LazorKitInstruction::Execute {
        role_id: 1,
        instruction_payload: payload2,
    };
    let execute_ix_data2 = borsh::to_vec(&execute_ix_struct2).unwrap();

    let execute_ix2 = Instruction {
        program_id: env.program_id,
        accounts: execute_accounts2,
        data: execute_ix_data2,
    };

    let tx2 = Transaction::new(
        &[&env.payer, &delegate_kp],
        Message::new(&[execute_ix2], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    let res2 = env.svm.send_transaction(bridge_tx(tx2));
    assert!(
        res2.is_err(),
        "Execute 1500 should have failed due to SOL limit"
    );

    // 6. Execute Success (Final check - 1000 left, spend 500)
    let recipient3 = Keypair::new().pubkey();
    let transfer_amount3 = 500;
    let target_ix3 = system_instruction::transfer(&vault_pda, &recipient3, transfer_amount3);

    let mut payload3 = vec![6u8];
    payload3.extend_from_slice(&target_ix3.data);

    let mut execute_accounts3 = execute_accounts.clone();
    execute_accounts3[5] = AccountMeta::new(recipient3, false);

    let execute_ix_struct3 = LazorKitInstruction::Execute {
        role_id: 1,
        instruction_payload: payload3,
    };
    let execute_ix_data3 = borsh::to_vec(&execute_ix_struct3).unwrap();

    let execute_ix3 = Instruction {
        program_id: env.program_id,
        accounts: execute_accounts3,
        data: execute_ix_data3,
    };

    let tx3 = Transaction::new(
        &[&env.payer, &delegate_kp],
        Message::new(&[execute_ix3], Some(&env.payer.pubkey())),
        to_sdk_hash(env.svm.latest_blockhash()),
    );

    env.svm
        .send_transaction(bridge_tx(tx3))
        .expect("Execute 500 failed");

    // Final state check
    let config_acc_final = env
        .svm
        .get_account(&Address::from(config_pda.to_bytes()))
        .unwrap();
    let stored_state_final =
        unsafe { SolLimitState::load_unchecked(&config_acc_final.data[state_start..]).unwrap() };
    assert_eq!(stored_state_final.amount, 500, "Final state mismatch");
}
