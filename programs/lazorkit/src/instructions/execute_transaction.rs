//! Execute a user-requested transaction on behalf of a smart-wallet.
//!
//! High-level flow
//! 1. `verify_authorization` – verifies passkey, signature, timestamp & nonce.
//! 2. Forward *rule* check instruction (must succeed) to the rule program.
//! 3. Depending on the desired action:
//!    • If the CPI data represents a SOL transfer → perform a lamport move
//!      directly with PDA authority (cheaper than a CPI).
//!    • Otherwise → invoke the target program via CPI, signing with the
//!      smart-wallet PDA.
//! 4. Increment the smart-wallet nonce.
//!
//! The account layout & args mirror the original monolithic implementation,
//! but the business logic now lives in smaller helpers for clarity.

// -----------------------------------------------------------------------------
//  Imports
// -----------------------------------------------------------------------------

use anchor_lang::prelude::*;

use crate::state::{Config, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms};
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer, sighash, transfer_sol_from_pda,
    verify_authorization, PasskeyExt, PdaSigner,
};
use crate::{
    constants::{SMART_WALLET_SEED, SOL_TRANSFER_DISCRIMINATOR},
    error::LazorKitError,
    ID,
};
use anchor_lang::solana_program::sysvar::instructions::ID as IX_ID;

use super::common::CpiData;

/// Arguments for the `execute_transaction` entrypoint (formerly `ExecuteCpi` action)
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteTransactionArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub rule_data: CpiData,
    pub cpi_data: Option<CpiData>,
}

/// Entrypoint for spending / minting tokens from the smart-wallet.
pub fn execute_transaction(
    mut ctx: Context<ExecuteTransaction>,
    args: ExecuteTransactionArgs,
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

    handle_execute_cpi(&mut ctx, &args)?;

    // Update nonce
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

fn handle_execute_cpi(
    ctx: &mut Context<ExecuteTransaction>,
    args: &ExecuteTransactionArgs,
) -> Result<()> {
    // Rule program whitelist check
    let rule_program_key = ctx.accounts.authenticator_program.key();
    check_whitelist(&ctx.accounts.whitelist_rule_programs, &rule_program_key)?;

    // Prepare PDA signer for rule CPI
    let rule_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.bumps.smart_wallet_authenticator,
    );
    let rule_accounts = &ctx.remaining_accounts[args.rule_data.start_index as usize
        ..(args.rule_data.start_index as usize + args.rule_data.length as usize)];

    // Rule instruction discriminator check
    require!(
        args.rule_data.data.get(0..8) == Some(&sighash("global", "check_rule")),
        LazorKitError::InvalidCheckRuleDiscriminator
    );

    // Execute rule CPI
    execute_cpi(
        rule_accounts,
        &args.rule_data.data,
        &ctx.accounts.authenticator_program,
        Some(rule_signer),
    )?;

    // --- CPI for main instruction ---
    let cpi_data = args
        .cpi_data
        .as_ref()
        .ok_or(LazorKitError::CpiDataMissing)?;
    let cpi_accounts = &ctx.remaining_accounts
        [cpi_data.start_index as usize..(cpi_data.start_index as usize + cpi_data.length as usize)];

    // Special handling for SOL transfer
    if cpi_data.data.get(0..4) == Some(&SOL_TRANSFER_DISCRIMINATOR)
        && ctx.accounts.cpi_program.key() == anchor_lang::solana_program::system_program::ID
    {
        require!(
            ctx.remaining_accounts.len() >= 2,
            LazorKitError::SolTransferInsufficientAccounts
        );
        let amount = u64::from_le_bytes(cpi_data.data[4..12].try_into().unwrap());
        transfer_sol_from_pda(
            &ctx.accounts.smart_wallet,
            &ctx.remaining_accounts[1].to_account_info(),
            amount,
        )?;
    } else {
        // Generic CPI with wallet signer
        let wallet_signer = PdaSigner {
            seeds: vec![
                SMART_WALLET_SEED.to_vec(),
                ctx.accounts.smart_wallet_config.id.to_le_bytes().to_vec(),
            ],
            bump: ctx.accounts.smart_wallet_config.bump,
        };
        execute_cpi(
            cpi_accounts,
            &cpi_data.data,
            &ctx.accounts.cpi_program,
            Some(wallet_signer),
        )?;
    }
    Ok(())
}

/// Accounts context for `execute_transaction`
#[derive(Accounts)]
#[instruction(args: ExecuteTransactionArgs)]
pub struct ExecuteTransaction<'info> {
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

    /// CHECK: Used for rule CPI.
    pub authenticator_program: UncheckedAccount<'info>,

    #[account(address = IX_ID)]
    /// CHECK: Sysvar for instructions.
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: Used for CPI, not deserialized.
    pub cpi_program: UncheckedAccount<'info>,
}
