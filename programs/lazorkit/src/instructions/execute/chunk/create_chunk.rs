use anchor_lang::prelude::*;

use crate::instructions::CreateChunkArgs;
use crate::security::validation;
use crate::state::{Chunk, Config, PolicyProgramRegistry, SmartWalletConfig, WalletDevice};
use crate::utils::{
    compute_create_chunk_message_hash, compute_instruction_hash, execute_cpi,
    get_wallet_device_signer, sighash, verify_authorization_hash, PasskeyExt,
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

pub fn create_chunk(ctx: Context<CreateChunk>, args: CreateChunkArgs) -> Result<()> {
    // Step 1: Validate input parameters and global program state
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_no_reentrancy(&ctx.remaining_accounts)?;
    validation::validate_policy_data(&args.policy_data)?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);

    // Step 2: Prepare policy program validation
    // In chunk mode, all remaining accounts are for policy checking
    let policy_accounts = &ctx.remaining_accounts[..];

    // Step 3: Validate policy program and verify data integrity
    // Ensure policy program is executable and matches wallet configuration
    validation::validate_program_executable(&ctx.accounts.policy_program)?;
    require!(
        ctx.accounts.policy_program.key() == ctx.accounts.smart_wallet_config.policy_program_id,
        LazorKitError::InvalidProgramAddress
    );

    // Verify policy program is registered in the whitelist
    crate::utils::check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.policy_program.key(),
    )?;

    // Step 4: Compute hashes for verification
    let policy_hash = compute_instruction_hash(
        &args.policy_data,
        policy_accounts,
        ctx.accounts.policy_program.key(),
    )?;

    let expected_message_hash = compute_create_chunk_message_hash(
        ctx.accounts.smart_wallet_config.last_nonce,
        args.timestamp,
        policy_hash,
        args.cpi_hash,
    )?;

    msg!("Expected message hash: {:?}", expected_message_hash);

    // Step 5: Verify WebAuthn signature and message hash
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

    // Step 5: Execute policy program validation
    // Create signer for policy program CPI
    let policy_signer = get_wallet_device_signer(
        &args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // Verify policy instruction discriminator
    require!(
        args.policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidCheckPolicyDiscriminator
    );

    // Execute policy program to validate the chunked transaction
    execute_cpi(
        policy_accounts,
        &args.policy_data,
        &ctx.accounts.policy_program,
        policy_signer,
    )?;

    // Step 6: Create the chunk buffer with authorization data
    let chunk: &mut Account<'_, Chunk> = &mut ctx.accounts.chunk;
    chunk.owner_wallet_address = ctx.accounts.smart_wallet.key();
    chunk.cpi_hash = args.cpi_hash;
    chunk.authorized_nonce = ctx.accounts.smart_wallet_config.last_nonce;
    chunk.authorized_timestamp = args.timestamp;
    chunk.rent_refund_address = ctx.accounts.payer.key();
    chunk.vault_index = args.vault_index;

    // Step 7: Update nonce after successful chunk creation
    ctx.accounts.smart_wallet_config.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.smart_wallet_config.last_nonce);

    msg!(
        "Successfully created chunk: wallet={}, nonce={}, cpi_hash={:?}",
        ctx.accounts.smart_wallet.key(),
        chunk.authorized_nonce,
        args.cpi_hash
    );
    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CreateChunkArgs)]
pub struct CreateChunk<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.wallet_id.to_le_bytes().as_ref()],
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

    #[account(
        seeds = [
            WalletDevice::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.passkey_public_key.to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump = wallet_device.bump,
        owner = ID,
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    /// CHECK: executable policy program
    #[account(executable)]
    pub policy_program: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Chunk::INIT_SPACE,
        seeds = [Chunk::PREFIX_SEED, smart_wallet.key().as_ref(), &smart_wallet_config.last_nonce.to_le_bytes()],
        bump,
        owner = ID,
    )]
    pub chunk: Account<'info, Chunk>,

    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
