use anchor_lang::prelude::*;

use crate::instructions::{Args as _, InvokePolicyArgs};
use crate::security::validation;
use crate::state::{Config, InvokePolicyMessage, PolicyProgramRegistry, SmartWallet, WalletDevice};
use crate::utils::{check_whitelist, execute_cpi, get_pda_signer, verify_authorization};
use crate::{error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn invoke_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, InvokePolicy<'info>>,
    args: InvokePolicyArgs,
) -> Result<()> {
    // 0. Validate args and global state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_program_executable(&ctx.accounts.policy_program)?;
    // Policy program must be the configured one and registered
    require!(
        ctx.accounts.policy_program.key() == ctx.accounts.smart_wallet_data.policy_program,
        LazorKitError::InvalidProgramAddress
    );
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.policy_program.key(),
    )?;
    validation::validate_policy_data(&args.policy_data)?;

    // Verify and deserialize message purpose-built for policy invocation
    let msg: InvokePolicyMessage = verify_authorization(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.wallet_device,
        ctx.accounts.smart_wallet.key(),
        args.passkey_pubkey,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        ctx.accounts.smart_wallet_data.last_nonce,
    )?;

    // Compare inline policy_data hash
    require!(
        hash(&args.policy_data).to_bytes() == msg.policy_data_hash,
        LazorKitError::InvalidInstructionData
    );

    // Hash policy accounts (skip optional new authenticator at index 0)
    let start_idx = if args.new_authenticator.is_some() {
        1
    } else {
        0
    };
    let policy_accs = &ctx.remaining_accounts[start_idx..];
    let mut hasher = Hasher::default();
    hasher.hash(ctx.accounts.policy_program.key().as_ref());
    for acc in policy_accs.iter() {
        hasher.hash(acc.key.as_ref());
        hasher.hash(&[acc.is_signer as u8]);
        hasher.hash(&[acc.is_writable as u8]);
    }
    require!(
        hasher.result().to_bytes() == msg.policy_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // PDA signer for policy CPI
    let policy_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
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
        crate::state::WalletDevice::init(
            new_auth,
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.smart_wallet.key(),
            new_authentcator.passkey_pubkey,
            new_authentcator.credential_id,
        )?;
    }

    // Execute policy CPI
    execute_cpi(
        policy_accs,
        &args.policy_data,
        &ctx.accounts.policy_program,
        Some(policy_signer),
        &[ctx.accounts.payer.key()],
    )?;

    // increment nonce
    ctx.accounts.smart_wallet_data.last_nonce = ctx
        .accounts
        .smart_wallet_data
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

#[derive(Accounts)]
pub struct InvokePolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [crate::constants::SMART_WALLET_SEED, smart_wallet_data.id.to_le_bytes().as_ref()],
        bump = smart_wallet_data.bump,
        owner = ID,
    )]
    /// CHECK: smart wallet PDA verified by seeds
    pub smart_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWallet::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_data: Box<Account<'info, SmartWallet>>,

    #[account(owner = ID)]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    /// CHECK: executable policy program
    #[account(executable)]
    pub policy_program: UncheckedAccount<'info>,

    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
