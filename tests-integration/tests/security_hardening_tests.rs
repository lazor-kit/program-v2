// Security Hardening Tests
// Tests for CPI Program Whitelist and Rent Exemption checks

mod common;
use common::*;
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

/// Test 1: CPI Whitelist - System Program (Allowed)
#[test]
fn test_cpi_whitelist_system_program_allowed() -> anyhow::Result<()> {
    let mut context = TestContext::new()?;

    // Create wallet
    let id = [1u8; 32];
    let (wallet_account, wallet_vault, root_authority) = create_lazorkit_wallet(&mut context, id)?;

    // Airdrop SOL to wallet_vault
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Airdrop failed: {:?}", e))?;

    // Create recipient
    let recipient = Keypair::new();
    let transfer_amount = 1 * LAMPORTS_PER_SOL;
    let inner_ix =
        system_instruction::transfer(&wallet_vault, &recipient.pubkey(), transfer_amount);

    // Use helper function to create Sign instruction
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &root_authority,
        0, // Root authority ID
        inner_ix,
    )?;

    // Execute transaction
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
            root_authority.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx);

    // Should succeed - System program is whitelisted
    assert!(
        result.is_ok(),
        "System program should be allowed: {:?}",
        result
    );

    // Verify transfer happened
    let recipient_balance = context
        .svm
        .get_account(&recipient.pubkey())
        .map(|acc| acc.lamports)
        .unwrap_or(0);
    assert_eq!(
        recipient_balance, transfer_amount,
        "Transfer should succeed"
    );

    Ok(())
}

/// Test 2: CPI Whitelist - Unauthorized Program (Blocked)
#[test]
fn test_cpi_whitelist_unauthorized_program_blocked() -> anyhow::Result<()> {
    let mut context = TestContext::new()?;

    // Create wallet
    let id = [2u8; 32];
    let (wallet_account, wallet_vault, root_authority) = create_lazorkit_wallet(&mut context, id)?;

    // Airdrop SOL to wallet_vault
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Airdrop failed: {:?}", e))?;

    // Create a restricted authority (ExecuteOnly) to enforce plugin/CPI checks
    let restricted_authority = Keypair::new();

    // We need RolePermission enum
    use lazorkit_v2_state::role_permission::RolePermission;

    let restricted_authority_id = common::add_authority_with_role_permission(
        &mut context,
        &wallet_account,
        &wallet_vault,
        &restricted_authority,
        0,               // acting_authority_id (root)
        &root_authority, // acting_authority
        RolePermission::ExecuteOnly,
    )?;

    // Create instruction to unauthorized program
    let unauthorized_program = Pubkey::new_unique();
    let malicious_ix = Instruction {
        program_id: unauthorized_program,
        accounts: vec![
            AccountMeta::new(wallet_vault, true), // wallet_vault as signer
        ],
        data: vec![],
    };

    // Use restricted authority to sign
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &restricted_authority,
        restricted_authority_id,
        malicious_ix,
    )?;

    // Execute transaction
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
            restricted_authority.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx);

    // Should fail with UnauthorizedCpiProgram error
    assert!(result.is_err(), "Unauthorized program should be blocked");

    // Check error contains UnauthorizedCpiProgram (error code 92)
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        // 41 = 0x29 = UnauthorizedCpiProgram
        assert!(
            error_msg.contains("41") || error_msg.contains("Custom(41)"),
            "Should fail with UnauthorizedCpiProgram error (41), got: {}",
            error_msg
        );
    }

    Ok(())
}

/// Test 3: Rent Exemption - Sufficient Balance
#[test]
fn test_rent_exemption_sufficient_balance() -> anyhow::Result<()> {
    let mut context = TestContext::new()?;

    // Create wallet
    let id = [3u8; 32];
    let (wallet_account, wallet_vault, root_authority) = create_lazorkit_wallet(&mut context, id)?;

    // Airdrop enough SOL to wallet_vault
    context
        .svm
        .airdrop(&wallet_vault, 10 * LAMPORTS_PER_SOL)
        .map_err(|e| anyhow::anyhow!("Airdrop failed: {:?}", e))?;

    // Get rent exemption minimum
    let wallet_vault_account = context.svm.get_account(&wallet_vault).unwrap();
    let rent_exempt_min = context
        .svm
        .minimum_balance_for_rent_exemption(wallet_vault_account.data.len());

    // Create recipient
    let recipient = Keypair::new();

    // Transfer amount that leaves enough for rent
    let transfer_amount = 10 * LAMPORTS_PER_SOL - rent_exempt_min - 1 * LAMPORTS_PER_SOL; // Leave 1 SOL buffer
    let inner_ix =
        system_instruction::transfer(&wallet_vault, &recipient.pubkey(), transfer_amount);

    // Use helper function to create Sign instruction
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &root_authority,
        0,
        inner_ix,
    )?;

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
            root_authority.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx);

    // Should succeed - enough balance for rent
    assert!(
        result.is_ok(),
        "Should succeed with sufficient balance for rent: {:?}",
        result
    );

    Ok(())
}

/// Test 4: Rent Exemption - Insufficient Balance (Wallet Vault)
#[test]
fn test_rent_exemption_insufficient_balance_vault() -> anyhow::Result<()> {
    let mut context = TestContext::new()?;

    // Create wallet
    let id = [4u8; 32];
    let (wallet_account, wallet_vault, root_authority) = create_lazorkit_wallet(&mut context, id)?;

    // Get rent exemption minimum
    let wallet_vault_account = context.svm.get_account(&wallet_vault).unwrap();
    let rent_exempt_min = context
        .svm
        .minimum_balance_for_rent_exemption(wallet_vault_account.data.len());

    // Airdrop minimal SOL to wallet_vault (just above rent minimum)
    // Note: airdrop ADDS to existing balance.
    // First, let's see what we start with.
    let initial_balance = context.svm.get_account(&wallet_vault).unwrap().lamports;

    // We want final balance to be rent_exempt_min + 1_000_000
    // So if we have initial_balance, we need to add: (rent_exempt_min + 1_000_000) - initial_balance
    // But since we can't easily subtract if initial is large, let's just make sure we interpret airdrop correctly.
    // Litesvm airdrop usually SETS account balance if I recall correctly, OR adds. Let's verify.
    // Actually, looking at litesvm docs or behavior, it usually adds or sets.
    // Let's just blindly add and then check.

    context
        .svm
        .airdrop(&wallet_vault, rent_exempt_min + 1_000_000)
        .map_err(|e| anyhow::anyhow!("Airdrop failed: {:?}", e))?;

    let balance_after_airdrop = context.svm.get_account(&wallet_vault).unwrap().lamports;

    // Create recipient
    let recipient = Keypair::new();

    // Try to transfer amount that would leave vault below rent exemption
    // We want to leave: rent_exempt_min - 100_000 (definitely below minimum)
    // So transfer_amount = balance_after_airdrop - (rent_exempt_min - 100_000)
    let transfer_amount = balance_after_airdrop - (rent_exempt_min - 100_000);
    // Just to be safe regarding arithmetic, let's just assert we have enough to transfer that much
    assert!(balance_after_airdrop > transfer_amount);

    let inner_ix =
        system_instruction::transfer(&wallet_vault, &recipient.pubkey(), transfer_amount);

    // Use helper function to create Sign instruction
    let sign_ix = create_sign_instruction_ed25519(
        &wallet_account,
        &wallet_vault,
        &root_authority,
        0,
        inner_ix,
    )?;

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
            root_authority.insecure_clone(),
        ],
    )?;

    let result = context.svm.send_transaction(tx);

    // Should fail with InsufficientBalance error
    assert!(result.is_err(), "Should fail with insufficient balance");

    // Check error is InsufficientBalance (error code 42)
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(
            error_msg.contains("42") || error_msg.contains("Custom(42)"),
            "Should fail with InsufficientBalance error (42), got: {}",
            error_msg
        );
    }

    Ok(())
}
