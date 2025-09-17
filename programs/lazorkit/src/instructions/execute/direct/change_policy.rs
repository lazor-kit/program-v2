use anchor_lang::prelude::*;

use crate::instructions::{Args as _, ChangePolicyArgs};
use crate::security::validation;
use crate::state::{
    LazorKitVault, PolicyProgramRegistry, Config, SmartWalletData,
    ChangePolicyMessage, WalletDevice,
};
use crate::utils::{
    check_whitelist, execute_cpi, get_wallet_device_signer, sighash, verify_authorization,
};
use crate::{error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

/// Change the policy program for a smart wallet
///
/// Allows changing the policy program that governs a smart wallet's transaction
/// validation rules. Requires proper WebAuthn authentication and validates that
/// both old and new policy programs are registered in the whitelist.
pub fn change_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ChangePolicy<'info>>,
    args: ChangePolicyArgs,
) -> Result<()> {
    // Step 1: Validate input arguments and global program state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    
    // Ensure both old and new policy programs are executable
    validation::validate_program_executable(&ctx.accounts.old_policy_program)?;
    validation::validate_program_executable(&ctx.accounts.new_policy_program)?;
    
    // Verify both policy programs are registered in the whitelist
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.old_policy_program.key(),
    )?;
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.new_policy_program.key(),
    )?;
    
    // Ensure the old policy program matches the wallet's current policy
    require!(
        ctx.accounts.smart_wallet_data.policy_program_id == ctx.accounts.old_policy_program.key(),
        LazorKitError::InvalidProgramAddress
    );
    
    // Ensure we're actually changing to a different policy program
    require!(
        ctx.accounts.old_policy_program.key() != ctx.accounts.new_policy_program.key(),
        LazorKitError::PolicyProgramsIdentical
    );
    
    // Validate policy instruction data sizes
    validation::validate_policy_data(&args.destroy_policy_data)?;
    validation::validate_policy_data(&args.init_policy_data)?;

    // Step 2: Verify WebAuthn signature and parse authorization message
    // This validates the passkey signature and extracts the typed message
    let msg: ChangePolicyMessage = verify_authorization(
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

    // Step 3: Split remaining accounts for destroy and init operations
    // Use split_index to separate accounts for the old and new policy programs
    let split = args.split_index as usize;
    require!(
        split <= ctx.remaining_accounts.len(),
        LazorKitError::AccountSliceOutOfBounds
    );

    // Adjust account slices if a new wallet device is being added
    let (destroy_accounts, init_accounts) = if args.new_wallet_device.is_some() {
        // Skip the first account (new wallet device) and split the rest
        let (destroy, init) = ctx.remaining_accounts[1..].split_at(split);
        (destroy, init)
    } else {
        // Split accounts directly for destroy and init operations
        ctx.remaining_accounts.split_at(split)
    };

    // Step 4: Verify account hashes match the authorization message
    // This ensures the accounts haven't been tampered with since authorization
    
    // Verify old policy program accounts hash
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

    // Verify new policy program accounts hash
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

    // Step 5: Verify instruction discriminators and data integrity
    // Ensure the policy data starts with the correct instruction discriminators
    require!(
        args.destroy_policy_data.get(0..8) == Some(&sighash("global", "destroy")),
        LazorKitError::InvalidDestroyDiscriminator
    );
    require!(
        args.init_policy_data.get(0..8) == Some(&sighash("global", "init_policy")),
        LazorKitError::InvalidInitPolicyDiscriminator
    );

    // Verify policy data hashes match the authorization message
    require!(
        hash(&args.destroy_policy_data).to_bytes() == msg.old_policy_data_hash,
        LazorKitError::InvalidInstructionData
    );
    require!(
        hash(&args.init_policy_data).to_bytes() == msg.new_policy_data_hash,
        LazorKitError::InvalidInstructionData
    );

    // Step 6: Prepare policy program signer and validate policy transition
    // Create a signer that can authorize calls to the policy programs
    let policy_signer = get_wallet_device_signer(
        &args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // Ensure at least one policy program is the default policy (security requirement)
    let default_policy = ctx.accounts.config.default_policy_program_id;
    require!(
        ctx.accounts.old_policy_program.key() == default_policy
            || ctx.accounts.new_policy_program.key() == default_policy,
        LazorKitError::NoDefaultPolicyProgram
    );

    // Step 7: Optionally create a new wallet device (passkey) if requested
    if let Some(new_wallet_device) = args.new_wallet_device {
        // Validate the new passkey format
        require!(
            new_wallet_device.passkey_public_key[0] == 0x02
                || new_wallet_device.passkey_public_key[0] == 0x03,
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

    // Step 8: Execute policy program transitions
    // First, destroy the old policy program state
    execute_cpi(
        destroy_accounts,
        &args.destroy_policy_data,
        &ctx.accounts.old_policy_program,
        policy_signer.clone(),
    )?;

    // Then, initialize the new policy program state
    execute_cpi(
        init_accounts,
        &args.init_policy_data,
        &ctx.accounts.new_policy_program,
        policy_signer,
    )?;

    // Step 9: Update wallet state after successful policy transition
    // Update the policy program ID to the new policy program
    ctx.accounts.smart_wallet_data.policy_program_id = ctx.accounts.new_policy_program.key();

    // Increment nonce to prevent replay attacks
    ctx.accounts.smart_wallet_data.last_nonce = ctx
        .accounts
        .smart_wallet_data
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    // Step 10: Handle fee distribution and vault validation
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

    // Distribute fees to payer, referral, and LazorKit vault
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

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

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
