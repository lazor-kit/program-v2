//! Swap the rule program linked to a smart-wallet.
//!
//! Steps
//! 1. Authorize request (passkey+signature).
//! 2. Validate whitelist & default rule constraints.
//! 3. Destroy old rule instance, init new one via CPIs.
//! 4. Persist the new `rule_program` in the smart-wallet config.
//! 5. Increment nonce.
//!
//! The destroy / init discriminators are checked to avoid accidental calls.

// -----------------------------------------------------------------------------
//  Imports
// -----------------------------------------------------------------------------

use anchor_lang::prelude::*;

use crate::state::{Config, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms};
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer, sighash, verify_authorization, PasskeyExt,
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};
use anchor_lang::solana_program::sysvar::instructions::ID as IX_ID;

use super::common::CpiData;

/// Arguments for swapping the rule program of a smart-wallet
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateRuleProgramArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub rule_data: CpiData,
    pub cpi_data: Option<CpiData>,
}

pub fn update_rule_program(
    mut ctx: Context<UpdateRuleProgram>,
    args: UpdateRuleProgramArgs,
) -> Result<()> {
    verify_authorization(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.smart_wallet_authenticator,
        ctx.accounts.smart_wallet.key(),
        args.passkey_pubkey,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        ctx.accounts.smart_wallet_config.last_nonce,
    )?;

    handle_change_program_rule(&mut ctx, &args)?;

    // Update nonce
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

fn handle_change_program_rule(
    ctx: &mut Context<UpdateRuleProgram>,
    args: &UpdateRuleProgramArgs,
) -> Result<()> {
    let old_rule_program_key = ctx.accounts.authenticator_program.key();
    let new_rule_program_key = ctx.accounts.cpi_program.key();
    let whitelist = &ctx.accounts.whitelist_rule_programs;
    let cpi_data = args
        .cpi_data
        .as_ref()
        .ok_or(LazorKitError::CpiDataMissing)?;

    check_whitelist(whitelist, &old_rule_program_key)?;
    check_whitelist(whitelist, &new_rule_program_key)?;

    // Destroy/init discriminators check
    require!(
        args.rule_data.data.get(0..8) == Some(&sighash("global", "destroy")),
        LazorKitError::InvalidDestroyDiscriminator
    );
    require!(
        cpi_data.data.get(0..8) == Some(&sighash("global", "init_rule")),
        LazorKitError::InvalidInitRuleDiscriminator
    );

    // Programs must differ
    require!(
        old_rule_program_key != new_rule_program_key,
        LazorKitError::RuleProgramsIdentical
    );

    // One of them must be the default
    let default_rule_program = ctx.accounts.config.default_rule_program;
    require!(
        old_rule_program_key == default_rule_program
            || new_rule_program_key == default_rule_program,
        LazorKitError::NoDefaultRuleProgram
    );

    // Update rule program in config
    ctx.accounts.smart_wallet_config.rule_program = new_rule_program_key;

    // Destroy old rule program
    let rule_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.bumps.smart_wallet_authenticator,
    );
    let rule_accounts = &ctx.remaining_accounts[args.rule_data.start_index as usize
        ..(args.rule_data.start_index as usize + args.rule_data.length as usize)];

    execute_cpi(
        rule_accounts,
        &args.rule_data.data,
        &ctx.accounts.authenticator_program,
        Some(rule_signer.clone()),
    )?;

    // Init new rule program
    let cpi_accounts = &ctx.remaining_accounts
        [cpi_data.start_index as usize..(cpi_data.start_index as usize + cpi_data.length as usize)];
    execute_cpi(
        cpi_accounts,
        &cpi_data.data,
        &ctx.accounts.cpi_program,
        Some(rule_signer),
    )?;
    Ok(())
}

/// Accounts context for `update_rule_program`
#[derive(Accounts)]
#[instruction(args: UpdateRuleProgramArgs)]
pub struct UpdateRuleProgram<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [Config::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.id.to_le_bytes().as_ref()],
        bump,
        owner = ID,
    )]
    /// CHECK: Only used for key and seeds.
    pub smart_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    #[account(
        seeds = [
            SmartWalletAuthenticator::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.passkey_pubkey.to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump,
        owner = ID,
    )]
    pub smart_wallet_authenticator: Box<Account<'info, SmartWalletAuthenticator>>,

    #[account(
        seeds = [WhitelistRulePrograms::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub whitelist_rule_programs: Box<Account<'info, WhitelistRulePrograms>>,

    /// CHECK: Old rule program (to be destroyed)
    pub authenticator_program: UncheckedAccount<'info>,

    #[account(address = IX_ID)]
    /// CHECK: Sysvar for instructions.
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: New rule program (to be initialised)
    pub cpi_program: UncheckedAccount<'info>,
}
