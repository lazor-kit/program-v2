use anchor_lang::prelude::*;

use crate::instructions::{Args as _, ExecuteArgs};
use crate::security::validation;
use crate::state::LazorKitVault;
use crate::utils::{
    check_whitelist, compute_execute_message_hash, compute_instruction_hash, execute_cpi,
    get_wallet_device_signer, sighash, split_remaining_accounts, verify_authorization_hash,
    PdaSigner,
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError};
// Hash and Hasher imports no longer needed with new verification approach

/// Execute a transaction through the smart wallet
///
/// The main transaction execution function that validates the transaction through
/// the policy program before executing the target program instruction. Supports
/// complex multi-instruction transactions with proper WebAuthn authentication.
pub fn execute<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
    args: ExecuteArgs,
) -> Result<()> {
    // Step 0: Validate input arguments and global program state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;

    // Step 0.1: Split remaining accounts between policy and CPI instructions
    // The split_index determines where to divide the accounts
    let (policy_accounts, cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, args.split_index)?;

    // Ensure we have accounts for the policy program
    require!(
        !policy_accounts.is_empty(),
        LazorKitError::InsufficientPolicyAccounts
    );

    // Step 0.2: Compute hashes for verification
    let policy_hash = compute_instruction_hash(
        &args.policy_data,
        policy_accounts,
        ctx.accounts.policy_program.key(),
    )?;

    let cpi_hash = compute_instruction_hash(
        &args.cpi_data,
        cpi_accounts,
        ctx.accounts.cpi_program.key(),
    )?;

    let expected_message_hash = compute_execute_message_hash(
        ctx.accounts.smart_wallet_config.last_nonce,
        args.timestamp,
        policy_hash,
        cpi_hash,
    )?;

    // Step 0.3: Verify WebAuthn signature and message hash
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.wallet_device,
        ctx.accounts.smart_wallet.key(),
        args.passkey_public_key,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;

    // Step 1: Validate and verify the policy program
    let policy_program_info = &ctx.accounts.policy_program;

    // Ensure the policy program is executable (not a data account)
    validation::validate_program_executable(policy_program_info)?;

    // Verify the policy program is registered in our whitelist
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &policy_program_info.key(),
    )?;

    // Ensure the policy program matches the wallet's configured policy
    require!(
        policy_program_info.key() == ctx.accounts.smart_wallet_config.policy_program_id,
        LazorKitError::InvalidProgramAddress
    );

    // Step 2: Prepare PDA signer for policy program CPI
    // Create a signer that can authorize calls to the policy program
    let policy_signer = get_wallet_device_signer(
        &args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // Step 3: Verify policy instruction discriminator and data integrity
    let policy_data = &args.policy_data;
    // Ensure the policy data starts with the correct instruction discriminator
    require!(
        policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidCheckPolicyDiscriminator
    );

    // Step 3.1: Validate policy data size
    validation::validate_policy_data(policy_data)?;

    // Step 5: Execute policy program CPI to validate the transaction
    // The policy program will check if this transaction is allowed based on
    // the wallet's security rules and return success/failure
    execute_cpi(
        policy_accounts,
        policy_data,
        policy_program_info,
        policy_signer,
    )?;

    // Step 6: Validate CPI instruction data
    validation::validate_cpi_data(&args.cpi_data)?;

    // Step 7: Execute the actual CPI instruction
    // Validate the target program is executable
    validation::validate_program_executable(&ctx.accounts.cpi_program)?;
    // Prevent reentrancy attacks by blocking calls to ourselves
    require!(
        ctx.accounts.cpi_program.key() != crate::ID,
        LazorKitError::ReentrancyDetected
    );
    // Ensure we have accounts for the CPI
    require!(
        !cpi_accounts.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );

    // Create PDA signer for the smart wallet to authorize the CPI
    let wallet_signer = PdaSigner {
        seeds: vec![
            SMART_WALLET_SEED.to_vec(),
            ctx.accounts
                .smart_wallet_config
                .wallet_id
                .to_le_bytes()
                .to_vec(),
        ],
        bump: ctx.accounts.smart_wallet_config.bump,
    };
    // Execute the actual transaction through CPI
    execute_cpi(
        cpi_accounts,
        &args.cpi_data,
        &ctx.accounts.cpi_program,
        wallet_signer.clone(),
    )?;

    // Step 8: Update wallet state and handle fees
    // Increment nonce to prevent replay attacks
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    // Validate that the provided vault matches the vault index from args
    crate::state::LazorKitVault::validate_vault_for_index(
        &ctx.accounts.lazorkit_vault.key(),
        args.vault_index,
        &crate::ID,
    )?;

    // Distribute fees to payer, referral, and LazorKit vault
    // This handles the fee distribution according to the configured rates
    crate::utils::distribute_fees(
        &ctx.accounts.config,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        wallet_signer,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: ExecuteArgs)]
pub struct Execute<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
    )]
    /// CHECK: PDA verified by seeds
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [crate::state::SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = crate::ID,
    )]
    pub smart_wallet_config: Box<Account<'info, crate::state::SmartWalletConfig>>,

    /// CHECK: referral account (matches smart_wallet_config.referral)
    #[account(mut, address = smart_wallet_config.referral_address)]
    pub referral: UncheckedAccount<'info>,

    /// LazorKit vault (empty PDA that holds SOL) - random vault selected by client
    #[account(
        mut,
        seeds = [LazorKitVault::PREFIX_SEED, &args.vault_index.to_le_bytes()],
        bump,
    )]
    /// CHECK: Empty PDA vault that only holds SOL, validated to be correct random vault
    pub lazorkit_vault: SystemAccount<'info>,

    #[account(owner = crate::ID)]
    pub wallet_device: Box<Account<'info, crate::state::WalletDevice>>,
    #[account(
        seeds = [crate::state::PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = crate::ID
    )]
    pub policy_program_registry: Box<Account<'info, crate::state::PolicyProgramRegistry>>,
    /// CHECK: must be executable (policy program)
    #[account(executable)]
    pub policy_program: UncheckedAccount<'info>,
    /// CHECK: must be executable (target program)
    #[account(executable)]
    pub cpi_program: UncheckedAccount<'info>,
    #[account(
        seeds = [crate::state::Config::PREFIX_SEED],
        bump,
        owner = crate::ID
    )]
    pub config: Box<Account<'info, crate::state::Config>>,
    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
