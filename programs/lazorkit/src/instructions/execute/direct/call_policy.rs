use anchor_lang::prelude::*;

use crate::instructions::{Args as _, CallPolicyArgs};
use crate::security::validation;
use crate::state::{Config, LazorKitVault, PolicyProgramRegistry, SmartWalletConfig, WalletDevice};
use crate::utils::{
    check_whitelist, compute_call_policy_message_hash, compute_instruction_hash, execute_cpi,
    get_wallet_device_signer, verify_authorization_hash,
};
use crate::{error::LazorKitError, ID};

pub fn call_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, CallPolicy<'info>>,
    args: CallPolicyArgs,
) -> Result<()> {
    // Step 1: Validate input arguments and global program state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_no_reentrancy(&ctx.remaining_accounts)?;

    // Ensure the policy program is executable (not a data account)
    validation::validate_program_executable(&ctx.accounts.policy_program)?;

    // Verify the policy program matches the wallet's configured policy
    require!(
        ctx.accounts.policy_program.key() == ctx.accounts.smart_wallet_config.policy_program_id,
        LazorKitError::InvalidProgramAddress
    );

    // Verify the policy program is registered in the whitelist
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.policy_program.key(),
    )?;

    // Validate policy instruction data size
    validation::validate_policy_data(&args.policy_data)?;

    // Step 2: Prepare policy accounts for verification
    // Skip the first account if a new wallet device is being added
    let start_idx = if args.new_wallet_device.is_some() {
        1
    } else {
        0
    };
    let policy_accs = &ctx.remaining_accounts[start_idx..];

    msg!("policy_accs: {:?}", policy_accs);

    // Step 3: Compute hashes for verification
    let policy_hash = compute_instruction_hash(
        &args.policy_data,
        policy_accs,
        ctx.accounts.policy_program.key(),
    )?;

    let expected_message_hash = compute_call_policy_message_hash(
        ctx.accounts.smart_wallet_config.last_nonce,
        args.timestamp,
        policy_hash,
    )?;

    // Step 4: Verify WebAuthn signature and message hash
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.wallet_device,
        ctx.accounts.smart_wallet.key(),
        args.passkey_public_key,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;

    // Step 5: Prepare policy program signer
    // Create a signer that can authorize calls to the policy program
    let policy_signer = get_wallet_device_signer(
        &args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // Step 6: Optionally create a new wallet device (passkey) if requested
    if let Some(new_wallet_device) = args.new_wallet_device {
        // Validate the new passkey format
        require!(
            new_wallet_device.passkey_public_key[0]
                == crate::constants::SECP256R1_COMPRESSED_PUBKEY_PREFIX_EVEN
                || new_wallet_device.passkey_public_key[0]
                    == crate::constants::SECP256R1_COMPRESSED_PUBKEY_PREFIX_ODD,
            LazorKitError::InvalidPasskeyFormat
        );

        // Get the new device account from remaining accounts
        let new_device = ctx
            .remaining_accounts
            .first()
            .ok_or(LazorKitError::InvalidRemainingAccounts)?;

        // Ensure the account is not already initialized
        require!(
            new_device.data_is_empty(),
            LazorKitError::AccountAlreadyInitialized
        );

        // Initialize the new wallet device
        crate::state::WalletDevice::init(
            new_device,
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.smart_wallet.key(),
            new_wallet_device.passkey_public_key,
            new_wallet_device.credential_id,
        )?;
    }

    // Step 7: Execute the policy program instruction
    execute_cpi(
        policy_accs,
        &args.policy_data,
        &ctx.accounts.policy_program,
        policy_signer,
    )?;

    // Step 8: Update wallet state and handle fees
    ctx.accounts.smart_wallet_config.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.smart_wallet_config.last_nonce);

    // Handle fee distribution and vault validation
    crate::utils::handle_fee_distribution(
        &ctx.accounts.config,
        &ctx.accounts.smart_wallet_config,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        args.vault_index,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CallPolicyArgs)]
pub struct CallPolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [crate::constants::SMART_WALLET_SEED, smart_wallet_config.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
    )]
    /// CHECK: PDA verified by seeds
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    /// CHECK: referral account (matches smart_wallet_config.referral)
    #[account(mut, address = smart_wallet_config.referral_address)]
    pub referral: UncheckedAccount<'info>,

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
