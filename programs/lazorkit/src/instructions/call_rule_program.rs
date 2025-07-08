use anchor_lang::solana_program::hash::hash;
use anchor_lang::{prelude::*, solana_program::sysvar::instructions::load_instruction_at_checked};

use crate::state::{Config, Message, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms};
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer,
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
    verify_passkey_and_signature(&ctx, &args, ctx.accounts.smart_wallet_config.last_nonce)?;

    handle_call_rule_program(&mut ctx, &args)?;

    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

fn verify_passkey_and_signature(
    ctx: &Context<CallRuleProgram>,
    args: &CallRuleProgramArgs,
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
    let rule_accounts = &ctx.remaining_accounts
        [args.rule_data.start_index as usize..(args.rule_data.start_index as usize + args.rule_data.length as usize)];
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
