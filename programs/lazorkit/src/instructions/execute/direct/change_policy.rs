use anchor_lang::prelude::*;

use crate::constants::SMART_WALLET_SEED;
use crate::instructions::{ChangePolicyArgs};
use crate::security::validation;
use crate::state::{Config, LazorKitVault, PolicyProgramRegistry, WalletDevice, WalletState};
use crate::utils::{
     compute_change_policy_message_hash, compute_instruction_hash, create_wallet_device_hash, execute_cpi, get_policy_signer, sighash, split_remaining_accounts, verify_authorization_hash
};
use crate::{error::LazorKitError, ID};

pub fn change_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ChangePolicy<'info>>,
    args: ChangePolicyArgs,
) -> Result<()> {
    require!(
        !ctx.accounts.lazorkit_config.is_paused,
        LazorKitError::ProgramPaused
    );

    let (destroy_accounts, init_accounts) = 
        split_remaining_accounts(&ctx.remaining_accounts, args.split_index)?;

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
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        args.passkey_public_key,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;

    require!(
        args.destroy_policy_data.get(0..8) == Some(&sighash("global", "destroy")),
        LazorKitError::InvalidDestroyDiscriminator
    );
    require!(
        args.init_policy_data.get(0..8) == Some(&sighash("global", "init_policy")),
        LazorKitError::InvalidInitPolicyDiscriminator
    );

    let policy_signer = get_policy_signer(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.key(),
        ctx.accounts.wallet_device.credential_hash,
    )?;
    execute_cpi(
        destroy_accounts,
        &args.destroy_policy_data,
        &ctx.accounts.old_policy_program,
        policy_signer.clone(),
    )?;
    execute_cpi(
        init_accounts,
        &args.init_policy_data,
        &ctx.accounts.new_policy_program,
        policy_signer,
    )?;

    // Update the policy program
    ctx.accounts.wallet_state.policy_program = ctx.accounts.new_policy_program.key();
    ctx.accounts.wallet_state.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.wallet_state.last_nonce);

    // Create the new wallet device account if it exists
    match args.new_wallet_device {
        Some(new_wallet_device_args) => {
            let new_wallet_device_account = &mut ctx.accounts.new_wallet_device.as_mut().unwrap();
            new_wallet_device_account.set_inner(WalletDevice {
                bump: ctx.bumps.new_wallet_device.unwrap(),
                passkey_pubkey: new_wallet_device_args.passkey_public_key,
                credential_hash: new_wallet_device_args.credential_hash,
                smart_wallet: ctx.accounts.smart_wallet.key(),
            });
        }
        _ => {}
    }
    
    // Handle fee distribution
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

    #[account(
        init,
        payer = payer,
        space = 8 + WalletDevice::INIT_SPACE,
        seeds = [WalletDevice::PREFIX_SEED, &create_wallet_device_hash(smart_wallet.key(), args.new_wallet_device.clone().unwrap().credential_hash)],
        bump
    )]
    pub new_wallet_device: Option<Box<Account<'info, WalletDevice>>>,

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
        constraint = old_policy_program.key() == wallet_state.policy_program @ LazorKitError::InvalidProgramAddress,
        constraint = policy_program_registry.registered_programs.contains(&old_policy_program.key()) @ LazorKitError::PolicyProgramNotRegistered
    )]
    /// CHECK: old policy program (executable)
    pub old_policy_program: UncheckedAccount<'info>,

    #[account(
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
