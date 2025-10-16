use anchor_lang::prelude::*;

use crate::constants::SMART_WALLET_SEED;
use crate::instructions::{Args as _, ChangePolicyArgs};
use crate::security::validation;
use crate::state::{Config, LazorKitVault, PolicyProgramRegistry, WalletDevice, WalletState};
use crate::utils::{
     compute_change_policy_message_hash, compute_instruction_hash, create_wallet_device_hash, execute_cpi, get_policy_signer, sighash, verify_authorization_hash
};
use crate::{error::LazorKitError, ID};

pub fn change_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ChangePolicy<'info>>,
    args: ChangePolicyArgs,
) -> Result<()> {
    // Step 1: Validate input arguments and global program state
    args.validate()?;
    require!(
        !ctx.accounts.lazorkit_config.is_paused,
        LazorKitError::ProgramPaused
    );
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_no_reentrancy(&ctx.remaining_accounts)?;

    // Validate policy instruction data sizes
    validation::validate_policy_data(&args.destroy_policy_data)?;
    validation::validate_policy_data(&args.init_policy_data)?;

    // Step 2: Split remaining accounts for destroy and init operations
    // Use split_index to separate accounts for the old and new policy programs
    let split = args.split_index as usize;
    require!(
        split <= ctx.remaining_accounts.len(),
        LazorKitError::AccountSliceOutOfBounds
    );

    // Adjust account slices if a new wallet device is being added
    let (destroy_accounts, init_accounts) = if args.new_wallet_device.is_some() {
        // Skip the first account (new wallet device) and split the rest
        let (destroy, init) = ctx.remaining_accounts[1..].split_at(split);
        (destroy, init)
    } else {
        // Split accounts directly for destroy and init operations
        ctx.remaining_accounts.split_at(split)
    };

    // Step 3: Compute hashes for verification
    let old_policy_hash = compute_instruction_hash(
        &args.destroy_policy_data,
        destroy_accounts,
        ctx.accounts.old_policy_program.key(),
    )?;

    let new_policy_hash = compute_instruction_hash(
        &args.init_policy_data,
        init_accounts,
        ctx.accounts.new_policy_program.key(),
    )?;

    let expected_message_hash = compute_change_policy_message_hash(
        ctx.accounts.wallet_state.last_nonce,
        args.timestamp,
        old_policy_hash,
        new_policy_hash,
    )?;

    // Step 4: Verify WebAuthn signature and message hash
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        args.passkey_public_key,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;

    // Step 5: Verify instruction discriminators and data integrity
    // Ensure the policy data starts with the correct instruction discriminators
    require!(
        args.destroy_policy_data.get(0..8) == Some(&sighash("global", "destroy")),
        LazorKitError::InvalidDestroyDiscriminator
    );
    require!(
        args.init_policy_data.get(0..8) == Some(&sighash("global", "init_policy")),
        LazorKitError::InvalidInitPolicyDiscriminator
    );

    // Step 6: Prepare policy program signer and validate policy transition
    // Create a signer that can authorize calls to the policy programs
    let policy_signer = get_policy_signer(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.key(),
        ctx.accounts.wallet_device.credential_hash,
    )?;

    // Ensure at least one policy program is the default policy (security requirement)
    let default_policy = ctx.accounts.lazorkit_config.default_policy_program_id;
    require!(
        ctx.accounts.old_policy_program.key() == default_policy
            || ctx.accounts.new_policy_program.key() == default_policy,
        LazorKitError::NoDefaultPolicyProgram
    );

    // Step 8: Execute policy program transitions
    // First, destroy the old policy program state
    execute_cpi(
        destroy_accounts,
        &args.destroy_policy_data,
        &ctx.accounts.old_policy_program,
        policy_signer.clone(),
    )?;

    // Then, initialize the new policy program state
    execute_cpi(
        init_accounts,
        &args.init_policy_data,
        &ctx.accounts.new_policy_program,
        policy_signer,
    )?;

    // Step 9: Update wallet state after successful policy transition
    ctx.accounts.wallet_state.policy_program = ctx.accounts.new_policy_program.key();
    ctx.accounts.wallet_state.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.wallet_state.last_nonce);

    // Step 10: Handle fee distribution and vault validation
    crate::utils::handle_fee_distribution(
        &ctx.accounts.lazorkit_config,
        &ctx.accounts.wallet_state,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        args.vault_index,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: ChangePolicyArgs)]
pub struct ChangePolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [Config::PREFIX_SEED],
        bump, 
        owner = ID
    )]
    pub lazorkit_config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, wallet_state.wallet_id.to_le_bytes().as_ref()],
        bump = wallet_state.bump,
    )]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        seeds = [WalletDevice::PREFIX_SEED, &create_wallet_device_hash(smart_wallet.key(), wallet_device.credential_hash)],
        bump,
        owner = ID,
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    #[account(mut, address = wallet_state.referral)]
    /// CHECK: referral account (matches wallet_state.referral)
    pub referral: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [LazorKitVault::PREFIX_SEED, &args.vault_index.to_le_bytes()],
        bump,
    )]
    /// CHECK: Empty PDA vault that only holds SOL, validated to be correct random vault
    pub lazorkit_vault: SystemAccount<'info>,

    #[account(
        executable,
        constraint = old_policy_program.key() == wallet_state.policy_program @ LazorKitError::InvalidProgramAddress,
        constraint = policy_program_registry.registered_programs.contains(&old_policy_program.key()) @ LazorKitError::PolicyProgramNotRegistered
    )]
    /// CHECK: old policy program (executable)
    pub old_policy_program: UncheckedAccount<'info>,

    #[account(
        executable,
        constraint = new_policy_program.key() != old_policy_program.key() @ LazorKitError::PolicyProgramsIdentical,
        constraint = policy_program_registry.registered_programs.contains(&new_policy_program.key()) @ LazorKitError::PolicyProgramNotRegistered
    )]
    /// CHECK: new policy program (executable)
    pub new_policy_program: UncheckedAccount<'info>,

    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
