//! Unified smart-wallet instruction dispatcher.
//!
//! External callers only need to invoke **one** instruction (`execute`) and
//! specify the desired `Action`.  Internally we forward to specialised
//! handler functions located in `handlers/`.

// -----------------------------------------------------------------------------
//  Imports
// -----------------------------------------------------------------------------
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::ID as IX_ID;

use crate::state::{Config, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms};
use crate::utils::{verify_authorization, PasskeyExt};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

use super::handlers::{call_rule, execute_tx, update_rule};

/// Supported wallet actions
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum Action {
    ExecuteTx,
    UpdateRuleProgram,
    CallRuleProgram,
}

/// Single args struct shared by all actions
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub action: Action,
    /// optional new authenticator passkey (only for `CallRuleProgram`)
    pub create_new_authenticator: Option<[u8; 33]>,
}

/// Single entry-point for all smart-wallet interactions
pub fn execute<'c: 'info, 'info>(
    mut ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
    args: ExecuteArgs,
) -> Result<()> {
    // ------------------------------------------------------------------
    // 1. Authorisation (shared)
    // ------------------------------------------------------------------
    let msg = verify_authorization(
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

    // ------------------------------------------------------------------
    // 2. Dispatch to specialised handler
    // ------------------------------------------------------------------
    match args.action {
        Action::ExecuteTx => {
            execute_tx::handle(&mut ctx, &args, &msg)?;
        }
        Action::UpdateRuleProgram => {
            update_rule::handle(&mut ctx, &args, &msg)?;
        }
        Action::CallRuleProgram => {
            call_rule::handle(&mut ctx, &args, &msg)?;
        }
    }

    // ------------------------------------------------------------------
    // 3. Increment nonce
    // ------------------------------------------------------------------
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

// -----------------------------------------------------------------------------
//  Anchor account context – superset of all action requirements
// -----------------------------------------------------------------------------
#[derive(Accounts)]
#[instruction(args: ExecuteArgs)]
pub struct Execute<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
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

    #[account(seeds = [WhitelistRulePrograms::PREFIX_SEED], bump, owner = ID)]
    pub whitelist_rule_programs: Box<Account<'info, WhitelistRulePrograms>>,

    /// CHECK: rule program being interacted with
    pub authenticator_program: UncheckedAccount<'info>,

    #[account(address = IX_ID)]
    /// CHECK: instruction sysvar
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: target program for CPI (if any)
    pub cpi_program: UncheckedAccount<'info>,

    // The new authenticator is an optional account that is only initialized
    // by the `CallRuleProgram` action. It is passed as an UncheckedAccount
    // and created via CPI if needed.
    pub new_smart_wallet_authenticator: Option<UncheckedAccount<'info>>,
}
