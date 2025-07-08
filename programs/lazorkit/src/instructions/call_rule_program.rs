//! Invoke an arbitrary instruction inside the *current* rule program.
//!
//! Optionally allows onboarding a **new authenticator** in the same
//! transaction. The caller must supply the new passkey pubkey; the PDA is
//! created with `init_if_needed`.
//!
//! Flow
//! 1. `verify_authorization`.
//! 2. (optional) create `new_smart_wallet_authenticator` PDA.
//! 3. Forward `rule_data` CPI signed by the *existing* authenticator PDA.
//! 4. Increment nonce.

// -----------------------------------------------------------------------------
//  Imports
// -----------------------------------------------------------------------------

use anchor_lang::prelude::*;

use crate::state::{Config, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms};
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer, verify_authorization, PasskeyExt,
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};
use anchor_lang::solana_program::sysvar::instructions::ID as IX_ID;

use super::common::CpiData;

/// Arguments for invoking a custom function on the rule program
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CallRuleProgramArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub rule_data: CpiData,
    /// Passkey for the *new* authenticator to be created (if any)
    pub create_new_authenticator: Option<[u8; 33]>,
}

pub fn call_rule_program(
    mut ctx: Context<CallRuleProgram>,
    args: CallRuleProgramArgs,
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

    handle_call_rule_program(&mut ctx, &args)?;

    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

fn handle_call_rule_program(
    ctx: &mut Context<CallRuleProgram>,
    args: &CallRuleProgramArgs,
) -> Result<()> {
    let rule_program_key = ctx.accounts.authenticator_program.key();
    check_whitelist(&ctx.accounts.whitelist_rule_programs, &rule_program_key)?;

    // Optionally create a new smart wallet authenticator
    if let Some(new_authenticator_pubkey) = args.create_new_authenticator {
        let new_auth = ctx
            .accounts
            .new_smart_wallet_authenticator
            .as_mut()
            .ok_or(LazorKitError::NewAuthenticatorMissing)?;
        new_auth.smart_wallet = ctx.accounts.smart_wallet.key();
        new_auth.passkey_pubkey = new_authenticator_pubkey;
        new_auth.bump = ctx.bumps.new_smart_wallet_authenticator.unwrap_or_default();
    } else {
        return Err(LazorKitError::NewAuthenticatorPasskeyMissing.into());
    }

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
        Some(rule_signer),
    )?;
    Ok(())
}

/// Accounts context for `call_rule_program`
#[derive(Accounts)]
#[instruction(args: CallRuleProgramArgs)]
pub struct CallRuleProgram<'info> {
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

    /// CHECK: rule program to be called
    pub authenticator_program: UncheckedAccount<'info>,

    #[account(address = IX_ID)]
    /// CHECK: Sysvar for instructions.
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: Not deserialized, just forwarded.
    pub cpi_program: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + SmartWalletAuthenticator::INIT_SPACE,
        seeds = [
            SmartWalletAuthenticator::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.create_new_authenticator.unwrap_or([0; 33]).to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump,
    )]
    pub new_smart_wallet_authenticator: Option<Account<'info, SmartWalletAuthenticator>>,
}
