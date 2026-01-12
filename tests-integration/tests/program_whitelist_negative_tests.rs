//! ProgramWhitelist Plugin Negative Tests
//!
//! Tests that verify ProgramWhitelist plugin correctly blocks non-whitelisted programs:
//! - ExecuteOnly authority với ProgramWhitelist plugin
//! - Transfer với whitelisted program → should pass ✅
//! - Transfer với non-whitelisted program → should fail ❌

mod common;
use common::*;
use lazorkit_v2_state::role_permission::RolePermission;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::VersionedTransaction,
};

// ============================================================================
// TEST: ProgramWhitelist Blocks Non-Whitelisted Program
// ============================================================================

/// Test ProgramWhitelist plugin blocks non-whitelisted programs
#[test_log::test]
#[ignore] // Access violation in LiteSVM when invoking plugin CPI
fn test_program_whitelist_blocks_non_whitelisted() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_authority_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;
    context
        .svm
        .airdrop(&wallet_vault, 100 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Step 1: Add authority with ExecuteOnly permission
    let spender_keypair = Keypair::new();
    let _spender_authority = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        0,
        &root_authority_keypair,
        RolePermission::ExecuteOnly,
    )?;

    // Step 2: Initialize and register ProgramWhitelist Plugin
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[root_authority_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    // Only whitelist System Program
    initialize_program_whitelist_plugin(
        &mut context,
        program_whitelist_program_id,
        &root_authority_keypair,
        &[solana_sdk::system_program::id()], // Only System Program whitelisted
    )?;

    add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        0u32, // Root authority ID
        program_whitelist_program_id,
        program_whitelist_config,
    )?;

    // Step 3: Link ProgramWhitelist plugin to Spender authority
    update_authority_with_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_authority_keypair,
        &spender_keypair.pubkey(),
        1, // Authority ID 1 (Spender)
        0, // Plugin Index 0
        10u8,
    )?;

    // Step 4: Test Spender can transfer với System Program (whitelisted) → should pass
    let recipient = Keypair::new();
    let recipient_pubkey = recipient.pubkey();
    let transfer_amount = 5 * LAMPORTS_PER_SOL;

    let inner_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);
    let mut sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &spender_keypair,
        1, // Authority ID 1 (Spender)
        inner_ix,
    )?;

    // Add plugin accounts
    sign_ix
        .accounts
        .push(AccountMeta::new(program_whitelist_config, false));
    sign_ix.accounts.push(AccountMeta::new_readonly(
        program_whitelist_program_id,
        false,
    ));

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            spender_keypair.insecure_clone(),
        ],
    )?;

    context.svm.send_transaction(tx).map_err(|e| {
        anyhow::anyhow!(
            "Failed to send transaction (System Program - whitelisted): {:?}",
            e
        )
    })?;

    // Step 5: Test Spender cannot transfer với non-whitelisted program → should fail
    // Create a dummy program instruction (not System Program)
    // We'll use a different program ID that's not whitelisted
    let dummy_program_id = Pubkey::new_unique();
    let dummy_instruction = Instruction {
        program_id: dummy_program_id,
        accounts: vec![
            AccountMeta::new(wallet_vault, false),
            AccountMeta::new(recipient_pubkey, false),
        ],
        data: vec![], // Empty data
    };

    // Build compact instruction for dummy program
    let accounts = vec![
        AccountMeta::new(wallet_account, false),
        AccountMeta::new(wallet_vault, false),
        AccountMeta::new_readonly(spender_keypair.pubkey(), true),
        AccountMeta::new_readonly(dummy_program_id, false), // Non-whitelisted program
        AccountMeta::new(recipient_pubkey, false),
    ];

    let mut instruction_payload = Vec::new();
    instruction_payload.push(1u8); // num_instructions
    instruction_payload.push(3u8); // dummy_program_id index (index 3)
    instruction_payload.push(dummy_instruction.accounts.len() as u8); // num_accounts
    instruction_payload.push(1u8); // wallet_vault index (index 1)
    instruction_payload.push(4u8); // recipient index (index 4)
    instruction_payload.extend_from_slice(&(dummy_instruction.data.len() as u16).to_le_bytes());
    instruction_payload.extend_from_slice(&dummy_instruction.data);

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Sign = 1
    instruction_data.extend_from_slice(&(instruction_payload.len() as u16).to_le_bytes());
    instruction_data.extend_from_slice(&1u32.to_le_bytes()); // authority_id = 1
    instruction_data.extend_from_slice(&instruction_payload);
    instruction_data.push(2u8); // authority_payload: [authority_index: 2]

    let sign_ix_fail = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };

    // Add plugin accounts
    let mut sign_ix_fail_with_plugin = sign_ix_fail;
    sign_ix_fail_with_plugin
        .accounts
        .push(AccountMeta::new(program_whitelist_config, false));
    sign_ix_fail_with_plugin
        .accounts
        .push(AccountMeta::new_readonly(
            program_whitelist_program_id,
            false,
        ));

    let message_fail = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            sign_ix_fail_with_plugin,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx_fail = VersionedTransaction::try_new(
        VersionedMessage::V0(message_fail),
        &[
            context.default_payer.insecure_clone(),
            spender_keypair.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx_fail);
    match result {
        Ok(_) => anyhow::bail!(
            "Transaction should have failed due to ProgramWhitelist (non-whitelisted program)"
        ),
        Err(_) => {
            // Expected failure
        },
    }

    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Add authority with role permission
fn add_authority_with_role_permission(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    wallet_vault: &Pubkey,
    new_authority: &Keypair,
    acting_authority_id: u32,
    acting_authority: &Keypair,
    role_permission: RolePermission,
) -> anyhow::Result<Pubkey> {
    let authority_hash = {
        let mut hasher = solana_sdk::hash::Hash::default();
        let mut hasher_state = hasher.to_bytes();
        hasher_state[..32].copy_from_slice(new_authority.pubkey().as_ref());
        solana_sdk::hash::hashv(&[&hasher_state]).to_bytes()
    };

    let seeds = wallet_authority_seeds(wallet_vault, &authority_hash);
    let (new_wallet_authority, _authority_bump) =
        Pubkey::find_program_address(&seeds, &lazorkit_program_id());

    let authority_data = new_authority.pubkey().to_bytes();
    let authority_data_len = authority_data.len() as u16;

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519 = 1
    instruction_data.extend_from_slice(&authority_data_len.to_le_bytes());
    instruction_data.extend_from_slice(&0u16.to_le_bytes()); // num_plugin_refs = 0
    instruction_data.push(role_permission as u8); // role_permission
    instruction_data.extend_from_slice(&[0u8; 3]); // padding
    instruction_data.extend_from_slice(&[0u8; 2]); // Alignment padding
    instruction_data.extend_from_slice(&authority_data);

    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload account: {:?}", e))?;

    let authority_payload_data = vec![4u8];
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload account: {:?}", e))?;

    let add_authority_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_account, false),
            AccountMeta::new(context.default_payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(authority_payload_pubkey, false),
            AccountMeta::new_readonly(acting_authority.pubkey(), true),
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");

    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            add_authority_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to add authority: {:?}", e))?;

    Ok(new_wallet_authority)
}

/// Initialize ProgramWhitelist plugin
fn initialize_program_whitelist_plugin(
    context: &mut TestContext,
    program_id: Pubkey,
    payer: &Keypair,
    whitelisted_programs: &[Pubkey],
) -> anyhow::Result<()> {
    let (config_pda, _bump) = Pubkey::find_program_address(&[payer.pubkey().as_ref()], &program_id);

    if context.svm.get_account(&config_pda).is_some() {
        return Ok(());
    }

    let estimated_size = 4 + (32 * whitelisted_programs.len()) + 1 + 8;
    let rent = context
        .svm
        .minimum_balance_for_rent_exemption(estimated_size);

    use solana_sdk::account::Account as SolanaAccount;
    let account = SolanaAccount {
        lamports: rent,
        data: vec![0u8; estimated_size],
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    };
    context.svm.set_account(config_pda, account).unwrap();

    use borsh::{BorshDeserialize, BorshSerialize};
    #[derive(BorshSerialize, BorshDeserialize)]
    enum PluginInstruction {
        CheckPermission,
        InitConfig { program_ids: Vec<[u8; 32]> },
        UpdateConfig,
    }

    let program_ids: Vec<[u8; 32]> = whitelisted_programs
        .iter()
        .map(|p| {
            let bytes = p.as_ref();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes[..32]);
            arr
        })
        .collect();
    let instruction = PluginInstruction::InitConfig { program_ids };
    let mut instruction_data = Vec::new();
    instruction
        .serialize(&mut instruction_data)
        .map_err(|e| anyhow::anyhow!("Failed to serialize: {:?}", e))?;

    let accounts = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(config_pda, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];

    let ix = Instruction {
        program_id,
        accounts,
        data: instruction_data,
    };

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            payer.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed init ProgramWhitelist plugin: {:?}", e))?;

    Ok(())
}

/// Update authority with plugin
fn update_authority_with_plugin(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    _wallet_vault: &Pubkey,
    acting_authority: &Keypair,
    authority_to_update: &Pubkey,
    authority_id: u32,
    plugin_index: u16,
    priority: u8,
) -> anyhow::Result<()> {
    let authority_data = authority_to_update.to_bytes();

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(6u16).to_le_bytes()); // UpdateAuthority = 6
    let acting_authority_id = 0u32; // Root
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519
    instruction_data.extend_from_slice(&(32u16).to_le_bytes()); // authority_data_len
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // num_plugin_refs = 1
    instruction_data.extend_from_slice(&[0u8; 2]); // padding

    instruction_data.extend_from_slice(&authority_data);

    // Plugin ref: [plugin_index: u16, priority: u8, enabled: u8, padding: [u8; 4]]
    instruction_data.extend_from_slice(&plugin_index.to_le_bytes());
    instruction_data.push(priority);
    instruction_data.push(1u8); // enabled
    instruction_data.extend_from_slice(&[0u8; 4]); // padding

    // Authority Payload for Ed25519
    let authority_payload = vec![3u8]; // Index of acting authority
    instruction_data.extend_from_slice(&authority_payload);

    let mut accounts = vec![
        AccountMeta::new(*wallet_account, false),
        AccountMeta::new(context.default_payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        AccountMeta::new_readonly(acting_authority.pubkey(), true),
    ];

    let ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    };

    let payer_pubkey = context.default_payer.pubkey();
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to update authority: {:?}", e))?;
    Ok(())
}
