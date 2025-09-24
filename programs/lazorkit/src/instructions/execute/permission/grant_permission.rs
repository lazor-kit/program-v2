use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::Hasher;

use crate::instructions::GrantPermissionArgs;
use crate::security::validation;
use crate::state::{
    Permission, Config, SmartWalletConfig,
    WalletDevice,
};
use crate::utils::{verify_authorization_hash, PasskeyExt, compute_grant_permission_message_hash};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

pub fn grant_permission(
    ctx: Context<GrantPermission>,
    args: GrantPermissionArgs,
) -> Result<()> {
    // Step 1: Validate input parameters and global program state
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_no_reentrancy(&ctx.remaining_accounts)?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);

    // Validate instruction data and split indices format
    // For n instructions, we need n-1 split indices to divide the accounts
    require!(
        !args.instruction_data_list.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );
    require!(
        args.instruction_data_list.len() == args.split_index.len() + 1,
        LazorKitError::InvalidInstructionData
    );

    // Step 2: Create combined hashes for verification
    // Hash all instruction data to verify integrity
    let serialized_cpi_data = args
        .instruction_data_list
        .try_to_vec()
        .map_err(|_| LazorKitError::InvalidInstructionData)?;
    let data_hash = anchor_lang::solana_program::hash::hash(&serialized_cpi_data).to_bytes();

    // Hash all accounts to verify they haven't been tampered with
    let mut all_accounts_hasher = Hasher::default();
    for acc in ctx.remaining_accounts.iter() {
        all_accounts_hasher.hash(acc.key.as_ref());
        all_accounts_hasher.hash(&[acc.is_signer as u8]);
        all_accounts_hasher.hash(&[acc.is_writable as u8]);
    }
    let accounts_hash = all_accounts_hasher.result().to_bytes();

    // Combine hashes
    let mut combined = Vec::new();
    combined.extend_from_slice(&data_hash);
    combined.extend_from_slice(&accounts_hash);
    let combined_hash = anchor_lang::solana_program::hash::hash(&combined).to_bytes();

    // Step 3: Compute expected message hash
    let expected_message_hash = compute_grant_permission_message_hash(
        ctx.accounts.smart_wallet_config.last_nonce,
        args.timestamp,
        args.ephemeral_public_key,
        args.expires_at,
        combined_hash,
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

    // Step 6: Validate expiration time constraints
    let now = Clock::get()?.unix_timestamp;
    require!(args.expires_at > now, LazorKitError::InvalidInstructionData);
    require!(
        args.expires_at <= now + 3600, // Maximum 1 hour from now
        LazorKitError::InvalidInstructionData
    );
    
    // Validate timestamp using standardized validation
    validation::validate_instruction_timestamp(args.timestamp)?;

    // Step 7: Validate account ranges using split indices
    let account_ranges = crate::utils::calculate_account_ranges(&ctx.remaining_accounts, &args.split_index)?;

    // Step 8: Validate each instruction's programs for security
    crate::utils::validate_programs_in_ranges(&ctx.remaining_accounts, &account_ranges)?;

    // Step 9: Create the ephemeral permission account
    // Store the authorization data for later use by execute_with_permission
    let authorization = &mut ctx.accounts.permission;
    authorization.owner_wallet_address = ctx.accounts.smart_wallet.key();
    authorization.ephemeral_public_key = args.ephemeral_public_key;
    authorization.expires_at = args.expires_at;
    authorization.fee_payer_address = ctx.accounts.payer.key();
    authorization.rent_refund_address = ctx.accounts.payer.key();
    authorization.vault_index = args.vault_index;
    authorization.instruction_data_hash = data_hash;
    authorization.accounts_metadata_hash = accounts_hash;

    // Step 10: Update wallet state
    ctx.accounts.smart_wallet_config.last_nonce = 
        validation::safe_increment_nonce(ctx.accounts.smart_wallet_config.last_nonce);

    msg!("Successfully granted permission: wallet={}, ephemeral_key={}, expires_at={}, instructions={}", 
         ctx.accounts.smart_wallet.key(), 
         args.ephemeral_public_key,
         args.expires_at,
         args.instruction_data_list.len());
    Ok(())
}

#[derive(Accounts)]
#[instruction(args: GrantPermissionArgs)]
pub struct GrantPermission<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
        
    )]
    /// CHECK: PDA verified
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    #[account(
        seeds = [
            WalletDevice::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.passkey_public_key.to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump = wallet_device.bump,
        owner = ID,
        constraint = wallet_device.smart_wallet_address == smart_wallet.key() @ LazorKitError::SmartWalletConfigMismatch,
        constraint = wallet_device.passkey_public_key == args.passkey_public_key @ LazorKitError::PasskeyMismatch
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    /// New ephemeral authorization account (rent payer: payer)
    #[account(
        init,
        payer = payer,
        space = 8 + Permission::INIT_SPACE,
        seeds = [Permission::PREFIX_SEED, smart_wallet.key().as_ref(), args.ephemeral_public_key.as_ref()],
        bump,
        owner = ID,
    )]
    pub permission: Account<'info, Permission>,

    /// CHECK: instructions sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
