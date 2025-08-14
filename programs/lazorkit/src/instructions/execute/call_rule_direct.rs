use anchor_lang::prelude::*;

use crate::instructions::{Args as _, CallRuleArgs};
use crate::security::validation;
use crate::state::{
    CallRuleMessage, Config, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms,
};
use crate::utils::{check_whitelist, execute_cpi, get_pda_signer, verify_authorization};
use crate::{error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn call_rule_direct<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, CallRuleDirect<'info>>,
    args: CallRuleArgs,
) -> Result<()> {
    // 0. Validate args and global state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_program_executable(&ctx.accounts.rule_program)?;
    // Rule program must be the configured one and whitelisted
    require!(
        ctx.accounts.rule_program.key() == ctx.accounts.smart_wallet_config.rule_program,
        LazorKitError::InvalidProgramAddress
    );
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &ctx.accounts.rule_program.key(),
    )?;
    validation::validate_rule_data(&args.rule_data)?;

    // Verify and deserialize message purpose-built for call-rule
    let msg: CallRuleMessage = verify_authorization(
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

    // Compare inline rule_data hash
    require!(
        hash(&args.rule_data).to_bytes() == msg.rule_data_hash,
        LazorKitError::InvalidInstructionData
    );

    // Hash rule accounts (skip optional new authenticator at index 0)
    let start_idx = if args.new_authenticator.is_some() {
        1
    } else {
        0
    };
    let rule_accs = &ctx.remaining_accounts[start_idx..];
    let mut hasher = Hasher::default();
    hasher.hash(ctx.accounts.rule_program.key().as_ref());
    for acc in rule_accs.iter() {
        hasher.hash(acc.key.as_ref());
        hasher.hash(&[acc.is_signer as u8]);
    }
    require!(
        hasher.result().to_bytes() == msg.rule_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // PDA signer for rule CPI
    let rule_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.bump,
    );

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

    // Execute rule CPI
    execute_cpi(
        rule_accs,
        &args.rule_data,
        &ctx.accounts.rule_program,
        Some(rule_signer),
    )?;

    // increment nonce
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

#[derive(Accounts)]
pub struct CallRuleDirect<'info> {
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
    /// CHECK: smart wallet PDA verified by seeds
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

    /// CHECK: executable rule program
    #[account(executable)]
    pub rule_program: UncheckedAccount<'info>,

    #[account(
        seeds = [WhitelistRulePrograms::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub whitelist_rule_programs: Box<Account<'info, WhitelistRulePrograms>>,

    /// Optional new authenticator to initialize when requested in message
    pub new_smart_wallet_authenticator: Option<UncheckedAccount<'info>>,

    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
