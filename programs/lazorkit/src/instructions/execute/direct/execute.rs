use anchor_lang::prelude::*;

use crate::instructions::ExecuteArgs;
use crate::security::validation;
use crate::state::{LazorKitVault, WalletDevice, WalletState};
use crate::utils::{
    compute_execute_message_hash, compute_instruction_hash, create_wallet_device_hash, execute_cpi,
    get_policy_signer, sighash, split_remaining_accounts, verify_authorization_hash, PdaSigner,
};
use crate::ID;
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError};

pub fn execute<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
    args: ExecuteArgs,
) -> Result<()> {
    require!(
        !ctx.accounts.lazorkit_config.is_paused,
        LazorKitError::ProgramPaused
    );

    let (policy_accounts, cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, args.split_index)?;

    // Compute hashes for verification
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
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        args.passkey_public_key,
        args.signature,
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;

    // CPI to validate the transaction
    let policy_signer = get_policy_signer(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.key(),
        ctx.accounts.wallet_device.credential_hash,
    )?;
    let policy_data = &args.policy_data;
    require!(
        policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidCheckPolicyDiscriminator
    );
    execute_cpi(
        policy_accounts,
        policy_data,
        &ctx.accounts.policy_program,
        policy_signer,
    )?;

    // CPI to execute the transaction
    let wallet_signer = PdaSigner {
        seeds: vec![
            SMART_WALLET_SEED.to_vec(),
            ctx.accounts.wallet_state.wallet_id.to_le_bytes().to_vec(),
        ],
        bump: ctx.accounts.wallet_state.bump,
    };
    execute_cpi(
        cpi_accounts,
        &args.cpi_data,
        &ctx.accounts.cpi_program,
        wallet_signer.clone(),
    )?;

    // Update the nonce
    ctx.accounts.wallet_state.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.wallet_state.last_nonce);

    // Handle fee distribution and vault validation
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
    pub lazorkit_vault: SystemAccount<'info>,

    #[account(
        seeds = [crate::state::PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub policy_program_registry: Box<Account<'info, crate::state::PolicyProgramRegistry>>,

    #[account(
        constraint = policy_program.key() == wallet_state.policy_program @ LazorKitError::InvalidProgramAddress,
        constraint = policy_program_registry.registered_programs.contains(&policy_program.key()) @ LazorKitError::PolicyProgramNotRegistered
    )]
    /// CHECK: must be executable (policy program)
    pub policy_program: UncheckedAccount<'info>,

    #[account(
        executable,
        constraint = !policy_program_registry.registered_programs.contains(&cpi_program.key()) @ LazorKitError::InvalidProgramAddress,
        constraint = cpi_program.key() != ID @ LazorKitError::ReentrancyDetected
    )]
    /// CHECK: must be executable (target program)
    pub cpi_program: UncheckedAccount<'info>,

    #[account(
        seeds = [crate::state::Config::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub lazorkit_config: Box<Account<'info, crate::state::Config>>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instruction sysvar
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
