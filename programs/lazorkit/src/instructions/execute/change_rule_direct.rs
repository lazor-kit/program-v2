use anchor_lang::prelude::*;

use crate::instructions::{Args as _, ChangeRuleArgs};
use crate::security::validation;
use crate::state::{
    ChangeRuleMessage, Config, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms,
};
use crate::utils::{check_whitelist, execute_cpi, get_pda_signer, sighash, verify_authorization};
use crate::{error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn change_rule_direct<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ChangeRuleDirect<'info>>,
    args: ChangeRuleArgs,
) -> Result<()> {
    // 0. Validate args and global state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_program_executable(&ctx.accounts.old_rule_program)?;
    validation::validate_program_executable(&ctx.accounts.new_rule_program)?;
    // Whitelist and config checks
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &ctx.accounts.old_rule_program.key(),
    )?;
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &ctx.accounts.new_rule_program.key(),
    )?;
    require!(
        ctx.accounts.smart_wallet_config.rule_program == ctx.accounts.old_rule_program.key(),
        LazorKitError::InvalidProgramAddress
    );
    // Ensure different programs
    require!(
        ctx.accounts.old_rule_program.key() != ctx.accounts.new_rule_program.key(),
        LazorKitError::RuleProgramsIdentical
    );
    validation::validate_rule_data(&args.destroy_rule_data)?;
    validation::validate_rule_data(&args.init_rule_data)?;

    let msg: ChangeRuleMessage = verify_authorization(
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

    // accounts layout: Use split_index from args to separate destroy and init accounts
    let split = args.split_index as usize;
    require!(
        split <= ctx.remaining_accounts.len(),
        LazorKitError::AccountSliceOutOfBounds
    );
    let (destroy_accounts, init_accounts) = ctx.remaining_accounts.split_at(split);

    // Hash checks
    let mut h1 = Hasher::default();
    h1.hash(ctx.accounts.old_rule_program.key().as_ref());
    for a in destroy_accounts.iter() {
        h1.hash(a.key.as_ref());
        h1.hash(&[a.is_signer as u8]);
    }
    require!(
        h1.result().to_bytes() == msg.old_rule_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    let mut h2 = Hasher::default();
    h2.hash(ctx.accounts.new_rule_program.key().as_ref());
    for a in init_accounts.iter() {
        h2.hash(a.key.as_ref());
        h2.hash(&[a.is_signer as u8]);
    }
    require!(
        h2.result().to_bytes() == msg.new_rule_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // discriminators
    require!(
        args.destroy_rule_data.get(0..8) == Some(&sighash("global", "destroy")),
        LazorKitError::InvalidDestroyDiscriminator
    );
    require!(
        args.init_rule_data.get(0..8) == Some(&sighash("global", "init_rule")),
        LazorKitError::InvalidInitRuleDiscriminator
    );

    // Compare rule data hashes from message
    require!(
        hash(&args.destroy_rule_data).to_bytes() == msg.old_rule_data_hash,
        LazorKitError::InvalidInstructionData
    );
    require!(
        hash(&args.init_rule_data).to_bytes() == msg.new_rule_data_hash,
        LazorKitError::InvalidInstructionData
    );

    // signer for CPI
    let rule_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.bump,
    );

    // enforce default rule transition if desired
    let default_rule = ctx.accounts.config.default_rule_program;
    require!(
        ctx.accounts.old_rule_program.key() == default_rule
            || ctx.accounts.new_rule_program.key() == default_rule,
        LazorKitError::NoDefaultRuleProgram
    );

    // update wallet config
    ctx.accounts.smart_wallet_config.rule_program = ctx.accounts.new_rule_program.key();

    // Optionally create new authenticator if requested
    if let Some(new_authentcator) = args.new_authenticator {
        require!(
            new_authentcator.passkey_pubkey[0] == 0x02
                || new_authentcator.passkey_pubkey[0] == 0x03,
            LazorKitError::InvalidPasskeyFormat
        );
        // Get the new authenticator account from remaining accounts
        let new_auth = ctx
            .remaining_accounts
            .first()
            .ok_or(LazorKitError::InvalidRemainingAccounts)?;

        require!(
            new_auth.data_is_empty(),
            LazorKitError::AccountAlreadyInitialized
        );
        crate::state::SmartWalletAuthenticator::init(
            new_auth,
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.smart_wallet.key(),
            new_authentcator.passkey_pubkey,
            new_authentcator.credential_id,
        )?;
    }

    // destroy and init
    execute_cpi(
        destroy_accounts,
        &args.destroy_rule_data,
        &ctx.accounts.old_rule_program,
        Some(rule_signer.clone()),
    )?;

    execute_cpi(
        init_accounts,
        &args.init_rule_data,
        &ctx.accounts.new_rule_program,
        Some(rule_signer),
    )?;

    // bump nonce
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

#[derive(Accounts)]
pub struct ChangeRuleDirect<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [crate::constants::SMART_WALLET_SEED, smart_wallet_config.id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
        owner = ID,
    )]
    /// CHECK: PDA verified by seeds
    pub smart_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    #[account(owner = ID)]
    pub smart_wallet_authenticator: Box<Account<'info, SmartWalletAuthenticator>>,

    /// CHECK
    pub old_rule_program: UncheckedAccount<'info>,
    /// CHECK
    pub new_rule_program: UncheckedAccount<'info>,

    #[account(
        seeds = [WhitelistRulePrograms::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub whitelist_rule_programs: Box<Account<'info, WhitelistRulePrograms>>,

    /// CHECK
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
