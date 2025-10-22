use anchor_lang::prelude::*;

use crate::instructions::CreateChunkArgs;
use crate::security::validation;
use crate::state::{Chunk, Config, PolicyProgramRegistry, WalletDevice, WalletState};
use crate::utils::{
    compute_create_chunk_message_hash, compute_instruction_hash, create_wallet_device_hash, execute_cpi, get_policy_signer, sighash, verify_authorization_hash
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

pub fn create_chunk(ctx: Context<CreateChunk>, args: CreateChunkArgs) -> Result<()> {
    require!(!ctx.accounts.lazorkit_config.is_paused, LazorKitError::ProgramPaused);

    // Verify the authorization hash
    let policy_hash = compute_instruction_hash(
        &args.policy_data,
        ctx.remaining_accounts,
        ctx.accounts.policy_program.key(),
    )?;
    let expected_message_hash = compute_create_chunk_message_hash(
        ctx.accounts.wallet_state.last_nonce,
        args.timestamp,
        policy_hash,
        args.cpi_hash,
    )?;
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        args.passkey_public_key,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;


    let policy_signer = get_policy_signer(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.key(),
        ctx.accounts.wallet_device.credential_hash,
    )?;
    require!(
        args.policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidCheckPolicyDiscriminator
    );
    execute_cpi(
        ctx.remaining_accounts,
        &args.policy_data,
        &ctx.accounts.policy_program,
        policy_signer,
    )?;

    // Initialize the chunk account
    let chunk_account = &mut ctx.accounts.chunk;
    chunk_account.set_inner(Chunk {
        owner_wallet_address: ctx.accounts.smart_wallet.key(),
        cpi_hash: args.cpi_hash,
        authorized_nonce: ctx.accounts.wallet_state.last_nonce,
        authorized_timestamp: args.timestamp,
        rent_refund_address: ctx.accounts.payer.key(),
        vault_index: args.vault_index,
    });

    // Update the nonce
    ctx.accounts.wallet_state.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.wallet_state.last_nonce);

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CreateChunkArgs)]
pub struct CreateChunk<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [Config::PREFIX_SEED], 
        bump, 
        owner = ID
    )]
    pub lazorkit_config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, wallet_state.wallet_id.to_le_bytes().as_ref()],
        bump = wallet_state.bump,
    )]
    /// CHECK: PDA verified by seeds
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        seeds = [WalletDevice::PREFIX_SEED, &create_wallet_device_hash(smart_wallet.key(), wallet_device.credential_hash)],

        bump,
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
    #[account(
        executable,
        constraint = policy_program.key() == wallet_state.policy_program @ LazorKitError::InvalidProgramAddress,
        constraint = policy_program_registry.registered_programs.contains(&policy_program.key()) @ LazorKitError::PolicyProgramNotRegistered
    )]
    pub policy_program: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Chunk::INIT_SPACE,
        seeds = [Chunk::PREFIX_SEED, smart_wallet.key().as_ref(), &wallet_state.last_nonce.to_le_bytes()],
        bump,
        owner = ID,
    )]
    pub chunk: Account<'info, Chunk>,

    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
