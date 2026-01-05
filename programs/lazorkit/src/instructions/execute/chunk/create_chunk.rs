use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::HASH_BYTES;

use crate::security::validation;
use crate::state::{Chunk, WalletAuthority, WalletState};
use crate::utils::{
    compute_create_chunk_message_hash, compute_instruction_hash, create_wallet_authority_hash,
    execute_cpi, get_wallet_authority, sighash, verify_authorization_hash,
};
use crate::{
    constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
    error::LazorKitError,
    ID,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateChunkArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub policy_data: Vec<u8>,
    pub timestamp: i64,
    pub cpi_hash: [u8; HASH_BYTES],
}

/// Create a chunk for deferred execution of large transactions
pub fn create_chunk(ctx: Context<CreateChunk>, args: CreateChunkArgs) -> Result<()> {
    validation::validate_instruction_timestamp(args.timestamp)?;

    let smart_wallet_key = ctx.accounts.smart_wallet.key();
    let wallet_authority_key = ctx.accounts.wallet_authority.key();
    let policy_program_key = ctx.accounts.policy_program.key();
    let credential_hash = ctx.accounts.wallet_authority.credential_hash;
    let last_nonce = ctx.accounts.wallet_state.last_nonce;
    let payer_key = ctx.accounts.payer.key();

    let policy_hash = compute_instruction_hash(
        &args.policy_data,
        ctx.remaining_accounts,
        policy_program_key,
    )?;
    let expected_message_hash =
        compute_create_chunk_message_hash(last_nonce, args.timestamp, policy_hash, args.cpi_hash)?;
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        args.passkey_public_key,
        args.signature,
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;

    let wallet_authority =
        get_wallet_authority(smart_wallet_key, wallet_authority_key, credential_hash)?;
    require!(
        args.policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidInstructionDiscriminator
    );
    execute_cpi(
        ctx.remaining_accounts,
        &args.policy_data,
        &ctx.accounts.policy_program,
        &wallet_authority,
    )?;

    ctx.accounts.chunk.set_inner(Chunk {
        owner_wallet_address: smart_wallet_key,
        cpi_hash: args.cpi_hash,
        authorized_nonce: last_nonce,
        authorized_timestamp: Clock::get()?.unix_timestamp,
        rent_refund_address: payer_key,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CreateChunkArgs)]
pub struct CreateChunk<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, &wallet_state.base_seed, &wallet_state.salt.to_le_bytes()],
        bump = wallet_state.bump,
    )]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        seeds = [WalletAuthority::PREFIX_SEED, &create_wallet_authority_hash(smart_wallet.key(), wallet_authority.credential_hash)],
        bump,
        owner = ID,
    )]
    pub wallet_authority: Box<Account<'info, WalletAuthority>>,

    #[account(address = wallet_state.policy_program)]
    /// CHECK: Validated to match wallet_state.policy_program
    pub policy_program: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = Chunk::DISCRIMINATOR.len() + Chunk::INIT_SPACE,
        seeds = [Chunk::PREFIX_SEED, smart_wallet.key().as_ref(), &wallet_state.last_nonce.to_le_bytes()],
        bump,
        owner = ID,
    )]
    pub chunk: Account<'info, Chunk>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: Instruction sysvar
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
