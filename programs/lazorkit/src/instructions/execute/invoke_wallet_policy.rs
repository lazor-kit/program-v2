use anchor_lang::prelude::*;

use crate::instructions::{Args as _, InvokeWalletPolicyArgs};
use crate::security::validation;
use crate::state::{
    InvokeWalletPolicyMessage, LazorKitVault, PolicyProgramRegistry, ProgramConfig,
    SmartWalletData, WalletDevice,
};
use crate::utils::{check_whitelist, execute_cpi, get_wallet_device_signer, verify_authorization};
use crate::{error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn invoke_wallet_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, InvokeWalletPolicy<'info>>,
    args: InvokeWalletPolicyArgs,
) -> Result<()> {
    // 0. Validate args and global state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_program_executable(&ctx.accounts.policy_program)?;
    // Policy program must be the configured one and registered
    require!(
        ctx.accounts.policy_program.key() == ctx.accounts.smart_wallet_data.policy_program_id,
        LazorKitError::InvalidProgramAddress
    );
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.policy_program.key(),
    )?;
    validation::validate_policy_data(&args.policy_data)?;

    // Verify and deserialize message purpose-built for policy invocation
    let msg: InvokeWalletPolicyMessage = verify_authorization(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.wallet_device,
        ctx.accounts.smart_wallet.key(),
        args.passkey_public_key,
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

    // Hash policy accounts (skip optional new wallet_device at index 0)
    let start_idx = if args.new_wallet_device.is_some() {
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
    let policy_signer = get_wallet_device_signer(
        &args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // Optionally create new wallet_device if requested
    if let Some(new_wallet_device) = args.new_wallet_device {
        require!(
            new_wallet_device.passkey_public_key[0] == 0x02
                || new_wallet_device.passkey_public_key[0] == 0x03,
            LazorKitError::InvalidPasskeyFormat
        );
        // Get the new wallet_device account from remaining accounts
        let new_device = ctx
            .remaining_accounts
            .first()
            .ok_or(LazorKitError::InvalidRemainingAccounts)?;

        require!(
            new_device.data_is_empty(),
            LazorKitError::AccountAlreadyInitialized
        );
        crate::state::WalletDevice::init(
            new_device,
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.smart_wallet.key(),
            new_wallet_device.passkey_public_key,
            new_wallet_device.credential_id,
        )?;
    }

    // Execute policy CPI
    execute_cpi(
        policy_accs,
        &args.policy_data,
        &ctx.accounts.policy_program,
        policy_signer,
    )?;

    // increment nonce
    ctx.accounts.smart_wallet_data.last_nonce = ctx
        .accounts
        .smart_wallet_data
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    // Validate that the provided vault matches the vault index from args
    crate::state::LazorKitVault::validate_vault_for_index(
        &ctx.accounts.lazorkit_vault.key(),
        args.vault_index,
        &crate::ID,
    )?;

    // Create wallet signer for fee distribution
    let wallet_signer = crate::utils::PdaSigner {
        seeds: vec![
            crate::constants::SMART_WALLET_SEED.to_vec(),
            ctx.accounts
                .smart_wallet_data
                .wallet_id
                .to_le_bytes()
                .to_vec(),
        ],
        bump: ctx.accounts.smart_wallet_data.bump,
    };

    // Distribute fees to payer, referral, and lazorkit vault
    crate::utils::distribute_fees(
        &ctx.accounts.config,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        wallet_signer,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: InvokeWalletPolicyArgs)]
pub struct InvokeWalletPolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [ProgramConfig::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, ProgramConfig>>,

    #[account(
        mut,
        seeds = [crate::constants::SMART_WALLET_SEED, smart_wallet_data.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_data.bump,
    )]
    /// CHECK: smart wallet PDA verified by seeds
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletData::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_data: Box<Account<'info, SmartWalletData>>,

    /// CHECK: referral account (matches smart_wallet_data.referral)
    #[account(mut, address = smart_wallet_data.referral_address)]
    pub referral: UncheckedAccount<'info>,

    /// LazorKit vault (empty PDA that holds SOL) - random vault selected by client
    #[account(
        mut,
        seeds = [LazorKitVault::PREFIX_SEED, &args.vault_index.to_le_bytes()],
        bump,
    )]
    /// CHECK: Empty PDA vault that only holds SOL, validated to be correct random vault
    pub lazorkit_vault: SystemAccount<'info>,

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
