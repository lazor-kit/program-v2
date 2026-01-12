//! Error Handling Tests for Lazorkit V2
//!
//! This module tests various error conditions and edge cases.

mod common;
use common::*;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_instruction,
    transaction::VersionedTransaction,
};

#[test_log::test]
fn test_invalid_instruction_discriminator() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault, _root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Try to call with invalid instruction discriminator (999)
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(999u16).to_le_bytes()); // Invalid discriminator

    let invalid_ix = Instruction {
        program_id: common::lazorkit_program_id(),
        accounts: vec![AccountMeta::new(wallet_account, false)],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            invalid_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone()],
    )?;

    let result = context.svm.send_transaction(tx);
    assert!(
        result.is_err(),
        "Invalid instruction discriminator should fail"
    );

    Ok(())
}

#[test_log::test]
fn test_invalid_accounts_length() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault, _root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Try to sign with insufficient accounts (Sign requires at least 2 accounts: wallet_account and wallet_vault)
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Sign = 1
    instruction_data.extend_from_slice(&(0u16).to_le_bytes()); // payload_len = 0
    instruction_data.extend_from_slice(&(0u32).to_le_bytes()); // authority_id = 0

    let invalid_ix = Instruction {
        program_id: common::lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(wallet_account, false),
            // Missing wallet_vault account - should fail
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            invalid_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone()],
    )?;

    let result = context.svm.send_transaction(tx);
    assert!(result.is_err(), "Invalid accounts length should fail");

    Ok(())
}

#[test_log::test]
fn test_invalid_wallet_discriminator() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (_wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Create an account with invalid discriminator
    let invalid_account_keypair = Keypair::new();
    let invalid_account_pubkey = invalid_account_keypair.pubkey();
    context
        .svm
        .airdrop(&invalid_account_pubkey, 1_000_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Create account with wrong discriminator
    use solana_sdk::account::Account as SolanaAccount;
    let invalid_data = vec![99u8; 100]; // Invalid discriminator (should be Discriminator::WalletAccount = 0)
    let account = SolanaAccount {
        lamports: 1_000_000_000,
        data: invalid_data,
        owner: common::lazorkit_program_id(),
        executable: false,
        rent_epoch: 0,
    };
    context.svm.set_account(invalid_account_pubkey, account)?;

    // Try to use this invalid account as wallet_account
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Sign = 1
    instruction_data.extend_from_slice(&(0u16).to_le_bytes()); // payload_len = 0
    instruction_data.extend_from_slice(&(0u32).to_le_bytes()); // authority_id = 0

    let invalid_ix = Instruction {
        program_id: common::lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(invalid_account_pubkey, false), // Invalid wallet account
            AccountMeta::new(wallet_vault, false),
            AccountMeta::new_readonly(root_keypair.pubkey(), true),
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            invalid_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            root_keypair.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx);
    assert!(result.is_err(), "Invalid wallet discriminator should fail");

    Ok(())
}

#[test_log::test]
fn test_owner_mismatch() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, _wallet_vault, _root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Create an account owned by system program instead of lazorkit program
    let invalid_account_keypair = Keypair::new();
    let invalid_account_pubkey = invalid_account_keypair.pubkey();
    context
        .svm
        .airdrop(&invalid_account_pubkey, 1_000_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Create account with correct discriminator but wrong owner
    use lazorkit_v2_state::Discriminator;
    use solana_sdk::account::Account as SolanaAccount;
    let mut invalid_data = vec![0u8; 100];
    invalid_data[0] = Discriminator::WalletAccount as u8; // Correct discriminator
    let account = SolanaAccount {
        lamports: 1_000_000_000,
        data: invalid_data,
        owner: solana_sdk::system_program::id(), // Wrong owner (should be lazorkit_program_id)
        executable: false,
        rent_epoch: 0,
    };
    context.svm.set_account(invalid_account_pubkey, account)?;

    // Try to use this account as wallet_account
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    instruction_data.extend_from_slice(&(0u32).to_le_bytes()); // acting_authority_id = 0

    let invalid_ix = Instruction {
        program_id: common::lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(invalid_account_pubkey, false), // Wrong owner
            AccountMeta::new(context.default_payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            invalid_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone()],
    )?;

    let result = context.svm.send_transaction(tx);
    assert!(result.is_err(), "Owner mismatch should fail");

    Ok(())
}

#[test_log::test]
fn test_insufficient_funds() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Create a recipient account
    let recipient_keypair = Keypair::new();
    let recipient_pubkey = recipient_keypair.pubkey();
    context
        .svm
        .airdrop(&recipient_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Try to transfer more than wallet has
    let transfer_amount = 1_000_000_000_000u64; // Way more than wallet has

    let transfer_ix =
        system_instruction::transfer(&wallet_vault, &recipient_pubkey, transfer_amount);

    // Create Sign instruction
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32, // Root authority ID
        transfer_ix,
    )?;

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
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
            root_keypair.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx);
    assert!(result.is_err(), "Insufficient funds should fail");

    Ok(())
}

#[test_log::test]
fn test_invalid_signature() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, _root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Create a different keypair (not the root authority)
    let wrong_keypair = Keypair::new();

    // Create recipient
    let recipient_keypair = Keypair::new();
    let recipient_pubkey = recipient_keypair.pubkey();
    context
        .svm
        .airdrop(&recipient_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Try to sign with wrong keypair
    let transfer_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, 1_000_000);

    // Create Sign instruction with wrong authority
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &wrong_keypair, // Wrong keypair
        0u32,           // Root authority ID (but using wrong keypair)
        transfer_ix,
    )?;

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
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
            wrong_keypair.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx);
    assert!(result.is_err(), "Invalid signature should fail");

    Ok(())
}

#[test_log::test]
fn test_permission_denied_add_authority() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add an authority with ExecuteOnly permission (cannot manage authorities)
    use lazorkit_v2_state::role_permission::RolePermission;
    let execute_only_keypair = Keypair::new();
    let execute_only_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &execute_only_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::ExecuteOnly,
    )?;

    // Try to add another authority using ExecuteOnly authority (should fail)
    let new_authority_keypair = Keypair::new();
    let result = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &new_authority_keypair,
        execute_only_authority_id, // ExecuteOnly authority trying to add authority
        &execute_only_keypair,     // ExecuteOnly keypair
        RolePermission::All,
    );

    assert!(
        result.is_err(),
        "ExecuteOnly authority should not be able to add authority"
    );

    Ok(())
}

#[test_log::test]
fn test_permission_denied_remove_authority() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add an authority with ExecuteOnly permission
    use lazorkit_v2_state::role_permission::RolePermission;
    let execute_only_keypair = Keypair::new();
    let execute_only_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &execute_only_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::ExecuteOnly,
    )?;

    // Add another authority to remove
    let target_keypair = Keypair::new();
    let target_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &target_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::All,
    )?;

    // Try to remove authority using ExecuteOnly authority (should fail)
    let result = remove_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        execute_only_authority_id, // ExecuteOnly authority trying to remove authority
        target_authority_id,
        &execute_only_keypair, // ExecuteOnly keypair
    );

    assert!(
        result.is_err(),
        "ExecuteOnly authority should not be able to remove authority"
    );

    Ok(())
}

#[test_log::test]
fn test_permission_denied_update_authority() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add an authority with ExecuteOnly permission
    use lazorkit_v2_state::role_permission::RolePermission;
    let execute_only_keypair = Keypair::new();
    let execute_only_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &execute_only_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::ExecuteOnly,
    )?;

    // Add another authority to update
    let target_keypair = Keypair::new();
    let target_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &target_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::All,
    )?;

    // Try to update authority using ExecuteOnly authority (should fail)
    let new_authority_data = Keypair::new().pubkey().to_bytes();
    let result = update_authority(
        &mut context,
        &wallet_account,
        &wallet_vault,
        execute_only_authority_id, // ExecuteOnly authority trying to update authority
        target_authority_id,
        &execute_only_keypair, // ExecuteOnly keypair
        &new_authority_data,
    );

    assert!(
        result.is_err(),
        "ExecuteOnly authority should not be able to update authority"
    );

    Ok(())
}

#[test_log::test]
fn test_permission_denied_add_plugin() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add an authority with AllButManageAuthority permission (cannot manage plugins)
    use lazorkit_v2_state::role_permission::RolePermission;
    let all_but_manage_keypair = Keypair::new();
    let all_but_manage_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &all_but_manage_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::AllButManageAuthority,
    )?;

    // Try to add plugin using AllButManageAuthority authority (should fail)
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[all_but_manage_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    let result = add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &all_but_manage_keypair,
        all_but_manage_authority_id, // AllButManageAuthority trying to add plugin
        program_whitelist_program_id,
        program_whitelist_config,
    );

    assert!(
        result.is_err(),
        "AllButManageAuthority should not be able to add plugin"
    );

    Ok(())
}

#[test_log::test]
fn test_permission_denied_remove_plugin() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add a plugin first (using root authority)
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[root_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    let plugin_index = add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32, // Root authority
        program_whitelist_program_id,
        program_whitelist_config,
    )?;

    // Add an authority with AllButManageAuthority permission (cannot manage plugins)
    use lazorkit_v2_state::role_permission::RolePermission;
    let all_but_manage_keypair = Keypair::new();
    let all_but_manage_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &all_but_manage_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::AllButManageAuthority,
    )?;

    // Try to remove plugin using AllButManageAuthority authority (should fail)
    let result = remove_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        all_but_manage_authority_id, // AllButManageAuthority trying to remove plugin
        plugin_index,
        &all_but_manage_keypair, // AllButManageAuthority keypair
    );

    assert!(
        result.is_err(),
        "AllButManageAuthority should not be able to remove plugin"
    );

    Ok(())
}

#[test_log::test]
fn test_permission_denied_update_plugin() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add a plugin first (using root authority)
    let program_whitelist_program_id = program_whitelist_program_id();
    let (program_whitelist_config, _) = Pubkey::find_program_address(
        &[root_keypair.pubkey().as_ref()],
        &program_whitelist_program_id,
    );

    let plugin_index = add_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &root_keypair,
        0u32, // Root authority
        program_whitelist_program_id,
        program_whitelist_config,
    )?;

    // Add an authority with AllButManageAuthority permission (cannot manage plugins)
    use lazorkit_v2_state::role_permission::RolePermission;
    let all_but_manage_keypair = Keypair::new();
    let all_but_manage_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &all_but_manage_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::AllButManageAuthority,
    )?;

    // Try to update plugin using AllButManageAuthority authority (should fail)
    let result = update_plugin(
        &mut context,
        &wallet_account,
        &wallet_vault,
        all_but_manage_authority_id, // AllButManageAuthority trying to update plugin
        plugin_index,
        false,                   // disabled
        0u8,                     // priority
        &all_but_manage_keypair, // AllButManageAuthority keypair
    );

    assert!(
        result.is_err(),
        "AllButManageAuthority should not be able to update plugin"
    );

    Ok(())
}

#[test_log::test]
fn test_permission_denied_sign() -> anyhow::Result<()> {
    let mut context = setup_test_context()?;
    let wallet_id = rand::random::<[u8; 32]>();
    let (wallet_account, wallet_vault, root_keypair) =
        create_lazorkit_wallet(&mut context, wallet_id)?;

    // Add an authority with ManageAuthority permission (cannot execute transactions)
    use lazorkit_v2_state::role_permission::RolePermission;
    let manage_authority_keypair = Keypair::new();
    let manage_authority_id = add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &manage_authority_keypair,
        0u32,          // Root authority
        &root_keypair, // Root keypair (acting)
        RolePermission::ManageAuthority,
    )?;

    // Create recipient
    let recipient_keypair = Keypair::new();
    let recipient_pubkey = recipient_keypair.pubkey();
    context
        .svm
        .airdrop(&recipient_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

    // Try to sign/execute transaction using ManageAuthority authority (should fail)
    let transfer_ix = system_instruction::transfer(&wallet_vault, &recipient_pubkey, 1_000_000);

    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &manage_authority_keypair,
        manage_authority_id, // ManageAuthority trying to execute
        transfer_ix,
    )?;

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
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
            manage_authority_keypair.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx);
    assert!(
        result.is_err(),
        "ManageAuthority should not be able to execute transactions"
    );

    Ok(())
}
