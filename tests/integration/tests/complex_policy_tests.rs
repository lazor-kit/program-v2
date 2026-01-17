mod common;
use common::{bridge_tx, create_wallet, setup_env, to_sdk_hash, LiteSVMConnection};
use lazorkit_sdk::basic::actions::{
    AddAuthorityBuilder, ExecuteBuilder, RegisterPolicyBuilder, UpdateAuthorityBuilder,
};
use lazorkit_sdk::basic::policy::PolicyConfigBuilder;
use lazorkit_sdk::basic::wallet::LazorWallet;
use lazorkit_sdk::state::{AuthorityType, LazorKitWallet, PolicyRegistryEntry};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;

#[test]
fn test_multi_policy_enforcement() {
    let mut env = setup_env();
    let wallet_id = [200u8; 32];
    let owner_kp = Keypair::new();
    let (config_pda, vault_pda) =
        create_wallet(&mut env, wallet_id, &owner_kp, AuthorityType::Ed25519);
    let wallet = LazorWallet::new(env.program_id, config_pda, vault_pda);

    // 1. Register SolLimit and Whitelist Policies
    // SolLimit is env.sol_limit_id_pubkey
    // Whitelist is likely needed. Let's register a whitelist policy too.
    // Assuming whitelist is available in tests via env or we can mock register it.
    // The integration env sets up sol_limit and whitelist programs.

    // Register SolLimit
    let reg_sol_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let builder = RegisterPolicyBuilder::new(env.program_id)
            .with_payer(env.payer.pubkey())
            .with_policy(env.sol_limit_id_pubkey);
        futures::executor::block_on(builder.build_transaction(&conn)).unwrap()
    };
    let mut signed_reg_sol = Transaction::new_unsigned(reg_sol_tx.message);
    signed_reg_sol
        .try_sign(&[&env.payer], to_sdk_hash(env.svm.latest_blockhash()))
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_reg_sol)).unwrap();

    // Register Whitelist (assuming env has it, or we use a dummy ID if we don't have the program loaded?
    // setup_env() usually loads both. Let's check common/mod.rs later. Assuming it exists.)
    // Wait, common/mod.rs setup_env might not export whitelist ID.
    // I'll assume I can use a second SolLimit as a "second policy" for structural testing,
    // OR I just focus on SolLimit + separate logic if Whitelist isn't easily available.
    // Actually, let's use the `AddAuthority` with SolLimit, then `UpdateAuthority` to add another one.

    // For this test, I'll use SolLimit. I'll configure it to 1000 lamports.
    let sol_limit_policy_state = 1000u64.to_le_bytes().to_vec();

    // 2. Add Authority with SolLimit
    let auth_kp = Keypair::new();
    let policy_config = PolicyConfigBuilder::new()
        .add_policy(env.sol_limit_id_pubkey, sol_limit_policy_state)
        .build();

    let registry_pda = Pubkey::find_program_address(
        &[
            PolicyRegistryEntry::SEED_PREFIX,
            &env.sol_limit_id_pubkey.to_bytes(),
        ],
        &env.program_id,
    )
    .0;

    let add_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let builder = AddAuthorityBuilder::new(&wallet)
            .with_authority(auth_kp.pubkey())
            .with_type(AuthorityType::Ed25519)
            .with_policy_config(policy_config)
            .with_authorization_data(vec![4])
            .with_registry(registry_pda)
            .with_authorizer(owner_kp.pubkey());
        futures::executor::block_on(builder.build_transaction(&conn, env.payer.pubkey())).unwrap()
    };
    let mut signed_add = Transaction::new_unsigned(add_tx.message);
    signed_add
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    env.svm.send_transaction(bridge_tx(signed_add)).unwrap();

    // 3. Execute Transfer <= 1000 (Should Success)
    let recipient = Keypair::new().pubkey();
    let transfer_ix = system_instruction::transfer(&vault_pda, &recipient, 500);
    let exec_tx_success = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let builder = ExecuteBuilder::new(&wallet)
            .with_role_id(1)
            .add_instruction(transfer_ix)
            .with_signer(auth_kp.pubkey())
            .with_policy(env.sol_limit_id_pubkey);
        futures::executor::block_on(builder.build_transaction(&conn, env.payer.pubkey())).unwrap()
    };
    let mut signed_exec_success = Transaction::new_unsigned(exec_tx_success.message);
    signed_exec_success
        .try_sign(
            &[&env.payer, &auth_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    let res = env.svm.send_transaction(bridge_tx(signed_exec_success));
    if let Err(e) = &res {
        println!("Transfer 500 Failed: {:?}", e);
    }
    assert!(res.is_ok());

    // 4. Update Authority to Add a Second Policy (e.g. Limit 200) - Effectively overwriting or adding?
    // Let's use UpdateOperation::AddPolicies (1).
    // Note: In LazorKit, multiple instances of SAME policy program might be allowed if they have different state?
    // Or maybe we add a different policy.

    // For now, let's test Update with AddPolicies.
    // We will add a stricter limit: 200 lamports.
    // If both run, both must pass.
    // Since we spent 500, we have 500 left on first limit.
    // New limit is 200.
    // If we try to spend 300:
    // - Limit 1 (500 left): OK.
    // - Limit 2 (200 left): FAIL.

    let policy_config_2 = PolicyConfigBuilder::new()
        .add_policy(env.sol_limit_id_pubkey, 200u64.to_le_bytes().to_vec())
        .build();

    // Payload for AddPolicies: [policies_config]
    // UpdateOperation 1 = AddPolicies.

    // Prepend count (1) to payload, as UpdateAuthority expects [count(4), policies...]
    let mut payload = (1u32).to_le_bytes().to_vec();
    payload.extend(policy_config_2);

    let update_tx = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let builder = UpdateAuthorityBuilder::new(&wallet)
            .with_acting_role(0)
            .with_target_role(1)
            .with_operation(1) // AddPolicies
            .with_payload(payload)
            .with_registry(registry_pda) // Same registry
            .with_authorizer(owner_kp.pubkey());
        futures::executor::block_on(builder.build_transaction(&conn, env.payer.pubkey())).unwrap()
    };
    let mut signed_update = Transaction::new_unsigned(update_tx.message);
    signed_update
        .try_sign(
            &[&env.payer, &owner_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    let res_update = env.svm.send_transaction(bridge_tx(signed_update));
    println!("Update Result: {:?}", res_update);
    assert!(res_update.is_ok());

    // 5. Verify State: Should have 2 policies.
    let acc = env
        .svm
        .get_account(&solana_address::Address::from(config_pda.to_bytes()))
        .unwrap();
    let data = acc.data;
    use lazorkit_sdk::advanced::types::Transmutable;

    let wallet_header =
        unsafe { LazorKitWallet::load_unchecked(&data[..LazorKitWallet::LEN]).unwrap() };

    let mut iterator =
        lazorkit_state::RoleIterator::new(&data, wallet_header.role_count, LazorKitWallet::LEN);

    let (_pos0, _, _) = iterator.next().unwrap();
    let (pos1, _, _) = iterator.next().unwrap();

    println!(
        "Role 1 Policies Check: expected 2, got {}",
        pos1.num_policies
    );
    assert_eq!(pos1.num_policies, 2);

    // 6. Execute Transfer 300.
    // Limit 1 (1000 total, 500 spent): 500 remaining. 300 is OK.
    // Limit 2 (200 total, 0 spent): 200 remaining. 300 is FAIL.
    // Should FAIL.

    let transfer_ix_fail = system_instruction::transfer(&vault_pda, &recipient, 300);
    let exec_tx_fail = {
        let conn = LiteSVMConnection { svm: &env.svm };
        let builder = ExecuteBuilder::new(&wallet)
            .with_role_id(1)
            .add_instruction(transfer_ix_fail)
            .with_signer(auth_kp.pubkey())
            .with_policy(env.sol_limit_id_pubkey);
        futures::executor::block_on(builder.build_transaction(&conn, env.payer.pubkey())).unwrap()
    };
    let mut signed_exec_fail = Transaction::new_unsigned(exec_tx_fail.message);
    signed_exec_fail
        .try_sign(
            &[&env.payer, &auth_kp],
            to_sdk_hash(env.svm.latest_blockhash()),
        )
        .unwrap();
    let res_fail = env.svm.send_transaction(bridge_tx(signed_exec_fail));
    println!("Execute 300 result: {:?}", res_fail);
    assert!(res_fail.is_err());
}
