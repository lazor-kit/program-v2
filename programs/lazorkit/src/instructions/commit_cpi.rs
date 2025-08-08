use anchor_lang::prelude::*;

use crate::security::validation;
use crate::state::{
    Config, CpiCommit, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms,
};
use crate::utils::{execute_cpi, get_pda_signer, sighash, verify_authorization, PasskeyExt};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CommitArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub rule_data: Option<Vec<u8>>,
    pub cpi_program: Pubkey,
    pub cpi_accounts_hash: [u8; 32],
    pub cpi_data_hash: [u8; 32],
    pub expires_at: i64,
}

pub fn commit_cpi(ctx: Context<CommitCpi>, args: CommitArgs) -> Result<()> {
    // Validate
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    if let Some(ref rule_data) = args.rule_data {
        validation::validate_rule_data(rule_data)?;
    }
    // No CPI bytes stored in commit mode

    // Program not paused
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);

    // Authorization
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

    // Optionally rule-check now (binds policy at commit time)
    if let Some(ref rule_data) = args.rule_data {
        // First part of remaining accounts are for the rule program
        let split = msg.split_index as usize;
        require!(
            split <= ctx.remaining_accounts.len(),
            LazorKitError::InvalidSplitIndex
        );
        let rule_accounts = &ctx.remaining_accounts[..split];
        // Ensure rule program matches config and whitelist
        validation::validate_program_executable(&ctx.accounts.authenticator_program)?;
        require!(
            ctx.accounts.authenticator_program.key()
                == ctx.accounts.smart_wallet_config.rule_program,
            LazorKitError::InvalidProgramAddress
        );
        crate::utils::check_whitelist(
            &ctx.accounts.whitelist_rule_programs,
            &ctx.accounts.authenticator_program.key(),
        )?;

        let rule_signer = get_pda_signer(
            &args.passkey_pubkey,
            ctx.accounts.smart_wallet.key(),
            ctx.accounts.smart_wallet_authenticator.bump,
        );
        // Ensure discriminator is check_rule
        require!(
            rule_data.get(0..8) == Some(&sighash("global", "check_rule")),
            LazorKitError::InvalidCheckRuleDiscriminator
        );
        execute_cpi(
            rule_accounts,
            rule_data,
            &ctx.accounts.authenticator_program,
            Some(rule_signer),
        )?;
    }

    // Write commit
    let commit = &mut ctx.accounts.cpi_commit;
    commit.owner_wallet = ctx.accounts.smart_wallet.key();
    commit.target_program = args.cpi_program;
    commit.data_hash = args.cpi_data_hash;
    commit.accounts_hash = args.cpi_accounts_hash;
    commit.authorized_nonce = ctx.accounts.smart_wallet_config.last_nonce;
    commit.expires_at = args.expires_at;
    commit.rent_refund_to = ctx.accounts.payer.key();

    // Advance nonce
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CommitArgs)]
pub struct CommitCpi<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
        owner = ID,
    )]
    /// CHECK: PDA verified
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
        bump = smart_wallet_authenticator.bump,
        owner = ID,
        constraint = smart_wallet_authenticator.smart_wallet == smart_wallet.key() @ LazorKitError::SmartWalletMismatch,
        constraint = smart_wallet_authenticator.passkey_pubkey == args.passkey_pubkey @ LazorKitError::PasskeyMismatch
    )]
    pub smart_wallet_authenticator: Box<Account<'info, SmartWalletAuthenticator>>,

    #[account(seeds = [WhitelistRulePrograms::PREFIX_SEED], bump, owner = ID)]
    pub whitelist_rule_programs: Box<Account<'info, WhitelistRulePrograms>>,

    /// Rule program for optional policy enforcement at commit time
    /// CHECK: validated via executable + whitelist
    pub authenticator_program: UncheckedAccount<'info>,

    /// New commit account (rent payer: payer)
    #[account(
        init,
        payer = payer,
        space = 8 + CpiCommit::INIT_SPACE,
        seeds = [CpiCommit::PREFIX_SEED, smart_wallet.key().as_ref(), &args.cpi_data_hash],
        bump,
        owner = ID,
    )]
    pub cpi_commit: Account<'info, CpiCommit>,

    /// CHECK: instructions sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
