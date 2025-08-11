use anchor_lang::prelude::*;

use crate::instructions::CommitArgs;
use crate::security::validation;
use crate::state::{
    Config, CpiCommit, ExecuteMessage, SmartWalletAuthenticator, SmartWalletConfig,
    WhitelistRulePrograms,
};
use crate::utils::{execute_cpi, get_pda_signer, sighash, verify_authorization, PasskeyExt};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn commit_cpi(ctx: Context<CommitCpi>, args: CommitArgs) -> Result<()> {
    // 0. Validate
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_rule_data(&args.rule_data)?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);

    // 1. Authorization -> typed ExecuteMessage
    let msg: ExecuteMessage = verify_authorization::<ExecuteMessage>(
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

    // 2. In commit mode, all remaining accounts are for rule checking
    let rule_accounts = &ctx.remaining_accounts[..];

    // 3. Optional rule-check now (bind policy & validate hashes)
    // Ensure rule program matches config and whitelist
    validation::validate_program_executable(&ctx.accounts.authenticator_program)?;
    require!(
        ctx.accounts.authenticator_program.key() == ctx.accounts.smart_wallet_config.rule_program,
        LazorKitError::InvalidProgramAddress
    );
    crate::utils::check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &ctx.accounts.authenticator_program.key(),
    )?;

    // Compare rule_data hash with message
    require!(
        hash(&args.rule_data).to_bytes() == msg.rule_data_hash,
        LazorKitError::InvalidInstructionData
    );
    // Compare rule_accounts hash with message
    let mut rh = Hasher::default();
    rh.hash(ctx.accounts.authenticator_program.key.as_ref());
    for a in rule_accounts.iter() {
        rh.hash(a.key.as_ref());
        rh.hash(&[a.is_writable as u8, a.is_signer as u8]);
    }
    require!(
        rh.result().to_bytes() == msg.rule_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // Execute rule check
    let rule_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.bump,
    );
    require!(
        args.rule_data.get(0..8) == Some(&sighash("global", "check_rule")),
        LazorKitError::InvalidCheckRuleDiscriminator
    );
    execute_cpi(
        rule_accounts,
        &args.rule_data,
        &ctx.accounts.authenticator_program,
        Some(rule_signer),
    )?;

    // 5. Write commit using hashes from message
    let commit = &mut ctx.accounts.cpi_commit;
    commit.owner_wallet = ctx.accounts.smart_wallet.key();
    commit.data_hash = msg.cpi_data_hash;
    commit.accounts_hash = msg.cpi_accounts_hash;
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

    #[account(
        seeds = [WhitelistRulePrograms::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub whitelist_rule_programs: Box<Account<'info, WhitelistRulePrograms>>,

    /// Rule program for optional policy enforcement at commit time
    /// CHECK: validated via executable + whitelist
    #[account(executable)]
    pub authenticator_program: UncheckedAccount<'info>,

    /// New commit account (rent payer: payer)
    #[account(
        init,
        payer = payer,
        space = 8 + CpiCommit::INIT_SPACE,
        seeds = [CpiCommit::PREFIX_SEED, smart_wallet.key().as_ref(), &smart_wallet_config.last_nonce.to_le_bytes()],
        bump,
        owner = ID,
    )]
    pub cpi_commit: Account<'info, CpiCommit>,

    /// CHECK: instructions sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
