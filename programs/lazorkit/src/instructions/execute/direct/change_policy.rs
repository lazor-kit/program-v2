use anchor_lang::prelude::*;

use crate::instructions::{Args as _, ChangePolicyArgs};
use crate::security::validation;
use crate::state::{
    LazorKitVault, PolicyProgramRegistry, ProgramConfig, SmartWalletData,
    UpdateWalletPolicyMessage, WalletDevice,
};
use crate::utils::{
    check_whitelist, execute_cpi, get_wallet_device_signer, sighash, verify_authorization,
};
use crate::{error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn change_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ChangePolicy<'info>>,
    args: ChangePolicyArgs,
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
        ctx.accounts.smart_wallet_data.policy_program_id == ctx.accounts.old_policy_program.key(),
        LazorKitError::InvalidProgramAddress
    );
    // Ensure different programs
    require!(
        ctx.accounts.old_policy_program.key() != ctx.accounts.new_policy_program.key(),
        LazorKitError::PolicyProgramsIdentical
    );
    validation::validate_policy_data(&args.destroy_policy_data)?;
    validation::validate_policy_data(&args.init_policy_data)?;

    let msg: UpdateWalletPolicyMessage = verify_authorization(
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

    // accounts layout: Use split_index from args to separate destroy and init accounts
    let split = args.split_index as usize;
    require!(
        split <= ctx.remaining_accounts.len(),
        LazorKitError::AccountSliceOutOfBounds
    );

    // If new authenticator is provided, adjust the account slices
    let (destroy_accounts, init_accounts) = if args.new_wallet_device.is_some() {
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
    let policy_signer = get_wallet_device_signer(
        &args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // enforce default policy transition if desired
    let default_policy = ctx.accounts.config.default_policy_program_id;
    require!(
        ctx.accounts.old_policy_program.key() == default_policy
            || ctx.accounts.new_policy_program.key() == default_policy,
        LazorKitError::NoDefaultPolicyProgram
    );

    // Optionally create new authenticator if requested
    if let Some(new_wallet_device) = args.new_wallet_device {
        require!(
            new_wallet_device.passkey_public_key[0] == 0x02
                || new_wallet_device.passkey_public_key[0] == 0x03,
            LazorKitError::InvalidPasskeyFormat
        );
        // Get the new authenticator account from remaining accounts
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

    // destroy old rule
    execute_cpi(
        destroy_accounts,
        &args.destroy_policy_data,
        &ctx.accounts.old_policy_program,
        policy_signer.clone(),
    )?;

    // init new rule
    execute_cpi(
        init_accounts,
        &args.init_policy_data,
        &ctx.accounts.new_policy_program,
        policy_signer,
    )?;

    // After both CPIs succeed, update the policy program for the smart wallet
    ctx.accounts.smart_wallet_data.policy_program_id = ctx.accounts.new_policy_program.key();

    // bump nonce
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
#[instruction(args: ChangePolicyArgs)]
pub struct ChangePolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [ProgramConfig::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, ProgramConfig>>,

    #[account(
        mut,
        seeds = [crate::constants::SMART_WALLET_SEED, smart_wallet_data.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_data.bump,
    )]
    /// CHECK: PDA verified by seeds
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
