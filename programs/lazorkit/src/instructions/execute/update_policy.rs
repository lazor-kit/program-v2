use anchor_lang::prelude::*;

use crate::instructions::{Args as _, UpdatePolicyArgs};
use crate::security::validation;
use crate::state::{Config, PolicyProgramRegistry, SmartWallet, UpdatePolicyMessage, WalletDevice};
use crate::utils::{check_whitelist, execute_cpi, get_pda_signer, sighash, verify_authorization};
use crate::{error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn update_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, UpdatePolicy<'info>>,
    args: UpdatePolicyArgs,
) -> Result<()> {
    // 0. Validate args and global state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_program_executable(&ctx.accounts.old_policy_program)?;
    validation::validate_program_executable(&ctx.accounts.new_policy_program)?;
    // Registry and config checks
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.old_policy_program.key(),
    )?;
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.new_policy_program.key(),
    )?;
    require!(
        ctx.accounts.smart_wallet_data.policy_program == ctx.accounts.old_policy_program.key(),
        LazorKitError::InvalidProgramAddress
    );
    // Ensure different programs
    require!(
        ctx.accounts.old_policy_program.key() != ctx.accounts.new_policy_program.key(),
        LazorKitError::PolicyProgramsIdentical
    );
    validation::validate_policy_data(&args.destroy_policy_data)?;
    validation::validate_policy_data(&args.init_policy_data)?;

    let msg: UpdatePolicyMessage = verify_authorization(
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

    // accounts layout: Use split_index from args to separate destroy and init accounts
    let split = args.split_index as usize;
    require!(
        split <= ctx.remaining_accounts.len(),
        LazorKitError::AccountSliceOutOfBounds
    );

    // If new authenticator is provided, adjust the account slices
    let (destroy_accounts, init_accounts) = if args.new_authenticator.is_some() {
        let (destroy, init) = ctx.remaining_accounts[1..].split_at(split);
        (destroy, init)
    } else {
        ctx.remaining_accounts.split_at(split)
    };

    // Hash checks
    let mut h1 = Hasher::default();
    h1.hash(ctx.accounts.old_policy_program.key().as_ref());
    for a in destroy_accounts.iter() {
        h1.hash(a.key.as_ref());
        h1.hash(&[a.is_signer as u8]);
        h1.hash(&[a.is_writable as u8]);
    }
    require!(
        h1.result().to_bytes() == msg.old_policy_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    let mut h2 = Hasher::default();
    h2.hash(ctx.accounts.new_policy_program.key().as_ref());
    for a in init_accounts.iter() {
        h2.hash(a.key.as_ref());
        h2.hash(&[a.is_signer as u8]);
        h2.hash(&[a.is_writable as u8]);
    }
    require!(
        h2.result().to_bytes() == msg.new_policy_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // discriminators
    require!(
        args.destroy_policy_data.get(0..8) == Some(&sighash("global", "destroy")),
        LazorKitError::InvalidDestroyDiscriminator
    );
    require!(
        args.init_policy_data.get(0..8) == Some(&sighash("global", "init_policy")),
        LazorKitError::InvalidInitPolicyDiscriminator
    );

    // Compare policy data hashes from message
    require!(
        hash(&args.destroy_policy_data).to_bytes() == msg.old_policy_data_hash,
        LazorKitError::InvalidInstructionData
    );
    require!(
        hash(&args.init_policy_data).to_bytes() == msg.new_policy_data_hash,
        LazorKitError::InvalidInstructionData
    );

    // signer for CPI
    let policy_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // enforce default policy transition if desired
    let default_policy = ctx.accounts.config.default_policy_program;
    require!(
        ctx.accounts.old_policy_program.key() == default_policy
            || ctx.accounts.new_policy_program.key() == default_policy,
        LazorKitError::NoDefaultPolicyProgram
    );

    // update wallet config
    ctx.accounts.smart_wallet_data.policy_program = ctx.accounts.new_policy_program.key();

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

    // destroy and init
    execute_cpi(
        destroy_accounts,
        &args.destroy_policy_data,
        &ctx.accounts.old_policy_program,
        Some(policy_signer.clone()),
        &[],
    )?;

    execute_cpi(
        init_accounts,
        &args.init_policy_data,
        &ctx.accounts.new_policy_program,
        Some(policy_signer),
        &[ctx.accounts.payer.key()],
    )?;

    // bump nonce
    ctx.accounts.smart_wallet_data.last_nonce = ctx
        .accounts
        .smart_wallet_data
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    // Update the policy program for the smart wallet
    ctx.accounts.smart_wallet_data.policy_program = ctx.accounts.new_policy_program.key();

    Ok(())
}

#[derive(Accounts)]
pub struct UpdatePolicy<'info> {
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
    /// CHECK: PDA verified by seeds
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

    /// CHECK: old policy program (executable)
    #[account(executable)]
    pub old_policy_program: UncheckedAccount<'info>,
    /// CHECK: new policy program (executable)
    #[account(executable)]
    pub new_policy_program: UncheckedAccount<'info>,

    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    /// CHECK
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
