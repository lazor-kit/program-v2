use anchor_lang::solana_program::hash::hash;
use anchor_lang::{prelude::*, solana_program::sysvar::instructions::load_instruction_at_checked};

use crate::state::{Config, Message, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms};
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer, sighash,
    verify_secp256r1_instruction, PasskeyExt, PdaSigner,
};
use crate::{
    constants::SMART_WALLET_SEED,
    error::LazorKitError,
    ID,
};
use anchor_lang::solana_program::sysvar::instructions::ID as IX_ID;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use super::common::CpiData;

const MAX_TIMESTAMP_DRIFT_SECONDS: i64 = 30;

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
    verify_passkey_and_signature(&ctx, &args, ctx.accounts.smart_wallet_config.last_nonce)?;

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

fn verify_passkey_and_signature(
    ctx: &Context<UpdateRuleProgram>,
    args: &UpdateRuleProgramArgs,
    last_nonce: u64,
) -> Result<()> {
    let authenticator = &ctx.accounts.smart_wallet_authenticator;

    require!(
        authenticator.passkey_pubkey == args.passkey_pubkey,
        LazorKitError::PasskeyMismatch
    );
    require!(
        authenticator.smart_wallet == ctx.accounts.smart_wallet.key(),
        LazorKitError::SmartWalletMismatch
    );

    let secp_ix = load_instruction_at_checked(
        args.verify_instruction_index as usize,
        &ctx.accounts.ix_sysvar,
    )?;

    let client_hash = hash(&args.client_data_json_raw);

    let mut message = Vec::with_capacity(args.authenticator_data_raw.len() + client_hash.as_ref().len());
    message.extend_from_slice(&args.authenticator_data_raw);
    message.extend_from_slice(client_hash.as_ref());

    let json_str = core::str::from_utf8(&args.client_data_json_raw)
        .map_err(|_| LazorKitError::ClientDataInvalidUtf8)?;
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).map_err(|_| LazorKitError::ClientDataJsonParseError)?;
    let challenge = parsed["challenge"]
        .as_str()
        .ok_or(LazorKitError::ChallengeMissing)?;

    let challenge_clean = challenge
        .trim_matches(|c| c == '"' || c == '\'' || c == '/' || c == ' ');
    let challenge_bytes = URL_SAFE_NO_PAD
        .decode(challenge_clean)
        .map_err(|_| LazorKitError::ChallengeBase64DecodeError)?;

    let msg = Message::try_from_slice(&challenge_bytes)
        .map_err(|_| LazorKitError::ChallengeDeserializationError)?;

    let now = Clock::get()?.unix_timestamp;
    if msg.current_timestamp < now.saturating_sub(MAX_TIMESTAMP_DRIFT_SECONDS) {
        return Err(LazorKitError::TimestampTooOld.into());
    }
    if msg.current_timestamp > now.saturating_add(MAX_TIMESTAMP_DRIFT_SECONDS) {
        return Err(LazorKitError::TimestampTooNew.into());
    }

    require!(msg.nonce == last_nonce, LazorKitError::NonceMismatch);

    verify_secp256r1_instruction(
        &secp_ix,
        authenticator.passkey_pubkey,
        message,
        args.signature.clone(),
    )
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
    let rule_accounts = &ctx.remaining_accounts
        [args.rule_data.start_index as usize..(args.rule_data.start_index as usize + args.rule_data.length as usize)];

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