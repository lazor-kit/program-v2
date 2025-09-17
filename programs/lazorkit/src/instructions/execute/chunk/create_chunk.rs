use anchor_lang::prelude::*;

use crate::instructions::CreateChunkArgs;
use crate::security::validation;
use crate::state::{
    Chunk, CreateChunkMessage, PolicyProgramRegistry, Config, SmartWalletConfig, WalletDevice,
};
use crate::utils::{
    execute_cpi, get_wallet_device_signer, sighash, verify_authorization, PasskeyExt,
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

/// Create a chunk buffer for large transactions
///
/// Creates a buffer for chunked transactions when the main execute transaction
/// exceeds size limits. Splits large transactions into smaller, manageable
/// chunks that can be processed separately while maintaining security.
pub fn create_chunk(ctx: Context<CreateChunk>, args: CreateChunkArgs) -> Result<()> {
    // Step 1: Validate input parameters and global program state
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_policy_data(&args.policy_data)?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);

    // Step 2: Verify WebAuthn signature and parse authorization message
    // This validates the passkey signature and extracts the typed message
    let msg: CreateChunkMessage = verify_authorization::<CreateChunkMessage>(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.wallet_device,
        ctx.accounts.smart_wallet.key(),
        args.passkey_public_key,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        ctx.accounts.smart_wallet_config.last_nonce,
    )?;

    // Step 3: Prepare policy program validation
    // In chunk mode, all remaining accounts are for policy checking
    let policy_accounts = &ctx.remaining_accounts[..];

    // Step 4: Validate policy program and verify data integrity
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

    // Verify policy data hash matches the authorization message
    require!(
        hash(&args.policy_data).to_bytes() == msg.policy_data_hash,
        LazorKitError::InvalidInstructionData
    );
    
    // Verify policy accounts hash matches the authorization message
    let mut rh = Hasher::default();
    rh.hash(ctx.accounts.policy_program.key.as_ref());
    for a in policy_accounts.iter() {
        rh.hash(a.key.as_ref());
        rh.hash(&[a.is_signer as u8]);
        rh.hash(&[a.is_writable as u8]);
    }
    require!(
        rh.result().to_bytes() == msg.policy_accounts_hash,
        LazorKitError::InvalidAccountData
    );

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
    // Store the hashes and metadata for later execution
    let session: &mut Account<'_, Chunk> = &mut ctx.accounts.chunk;
    session.owner_wallet_address = ctx.accounts.smart_wallet.key();
    session.instruction_data_hash = msg.cpi_data_hash;
    session.accounts_metadata_hash = msg.cpi_accounts_hash;
    session.authorized_nonce = ctx.accounts.smart_wallet_config.last_nonce;
    session.expires_at = args.expires_at;
    session.rent_refund_address = ctx.accounts.payer.key();
    session.vault_index = args.vault_index;

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
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    /// Policy program for optional policy enforcement at session creation
    /// CHECK: validated via executable + registry
    #[account(executable)]
    pub policy_program: UncheckedAccount<'info>,

    /// New transaction session account (rent payer: payer)
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Chunk::INIT_SPACE,
        seeds = [Chunk::PREFIX_SEED, smart_wallet.key().as_ref(), &smart_wallet_config.last_nonce.to_le_bytes()],
        bump,
        owner = ID,
    )]
    pub chunk: Account<'info, Chunk>,

    /// CHECK: instructions sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
