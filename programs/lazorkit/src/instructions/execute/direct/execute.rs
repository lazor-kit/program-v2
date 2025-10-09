use anchor_lang::prelude::*;

use crate::instructions::{Args as _, ExecuteArgs};
use crate::security::validation;
use crate::state::{LazorKitVault, WalletState};
use crate::utils::{
    check_whitelist, compute_execute_message_hash, compute_instruction_hash, execute_cpi,
    hash_seeds, sighash, split_remaining_accounts, verify_authorization_hash, PdaSigner,
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError};

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
    validation::validate_no_reentrancy(&ctx.remaining_accounts)?;

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

    let cpi_hash =
        compute_instruction_hash(&args.cpi_data, cpi_accounts, ctx.accounts.cpi_program.key())?;

    let expected_message_hash = compute_execute_message_hash(
        ctx.accounts.wallet_state.last_nonce,
        args.timestamp,
        policy_hash,
        cpi_hash,
    )?;

    // Step 0.3: Verify WebAuthn signature and message hash
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
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
        policy_program_info.key() == ctx.accounts.wallet_state.policy_program,
        LazorKitError::InvalidProgramAddress
    );

    // Step 2: Prepare PDA signer for policy program CPI

    let seeds = &[&hash_seeds(
        &args.passkey_public_key.clone(),
        ctx.accounts.smart_wallet.key(),
    )[..]];

    let (_, bump) = Pubkey::find_program_address(seeds, &crate::ID);

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
        PdaSigner {
            seeds: vec![seeds[0].to_vec()],
            bump,
        },
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
            ctx.accounts.wallet_state.wallet_id.to_le_bytes().to_vec(),
        ],
        bump: ctx.accounts.wallet_state.bump,
    };
    // Execute the actual transaction through CPI
    execute_cpi(
        cpi_accounts,
        &args.cpi_data,
        &ctx.accounts.cpi_program,
        wallet_signer.clone(),
    )?;

    // Step 8: Update wallet state and handle fees
    ctx.accounts.wallet_state.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.wallet_state.last_nonce);

    // Handle fee distribution and vault validation
    crate::utils::handle_fee_distribution(
        &ctx.accounts.config,
        &ctx.accounts.wallet_state,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        args.vault_index,
    )?;

    msg!(
        "Successfully executed transaction: wallet={}, nonce={}, policy={}, cpi={}",
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_state.last_nonce,
        ctx.accounts.policy_program.key(),
        ctx.accounts.cpi_program.key()
    );
    Ok(())
}

#[derive(Accounts)]
#[instruction(args: ExecuteArgs)]
pub struct Execute<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, wallet_state.wallet_id.to_le_bytes().as_ref()],
        bump = wallet_state.bump,
    )]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [WalletState::PREFIX_SEED, wallet_state.wallet_id.to_le_bytes().as_ref()],
        bump,
        owner = crate::ID,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    /// CHECK: PDA verified by seeds
    pub wallet_signer: UncheckedAccount<'info>,

    #[account(mut, address = wallet_state.referral)]
    /// CHECK: referral account (matches wallet_state.referral)
    pub referral: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [LazorKitVault::PREFIX_SEED, &args.vault_index.to_le_bytes()],
        bump,
    )]
    pub lazorkit_vault: SystemAccount<'info>,

    #[account(
        seeds = [crate::state::PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = crate::ID
    )]
    pub policy_program_registry: Box<Account<'info, crate::state::PolicyProgramRegistry>>,

    #[account(executable)]
    /// CHECK: must be executable (policy program)
    pub policy_program: UncheckedAccount<'info>,

    #[account(executable)]
    /// CHECK: must be executable (target program)
    pub cpi_program: UncheckedAccount<'info>,

    #[account(
        seeds = [crate::state::Config::PREFIX_SEED],
        bump,
        owner = crate::ID
    )]
    pub config: Box<Account<'info, crate::state::Config>>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instruction sysvar
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
