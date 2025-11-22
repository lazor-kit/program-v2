use crate::constants::PASSKEY_PUBLIC_KEY_SIZE;
use crate::security::validation;
use crate::state::{WalletDevice, WalletState};
use crate::utils::{
    compute_execute_message_hash, compute_instruction_hash, create_wallet_device_hash, execute_cpi,
    get_policy_signer, sighash, split_remaining_accounts, verify_authorization_hash, PdaSigner,
};
use crate::ID;
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub policy_data: Vec<u8>,
    pub cpi_data: Vec<u8>,
    pub timestamp: i64,
}

/// Execute a transaction directly with passkey authentication
pub fn execute<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
    args: ExecuteArgs,
) -> Result<()> {
    validation::validate_instruction_timestamp(args.timestamp)?;

    let smart_wallet_key = ctx.accounts.smart_wallet.key();
    let wallet_device_key = ctx.accounts.wallet_device.key();
    let policy_program_key = ctx.accounts.policy_program.key();
    let cpi_program_key = ctx.accounts.cpi_program.key();
    let credential_hash = ctx.accounts.wallet_device.credential_hash;
    let wallet_id = ctx.accounts.wallet_state.wallet_id;
    let wallet_bump = ctx.accounts.wallet_state.bump;
    let last_nonce = ctx.accounts.wallet_state.last_nonce;

    let (policy_accounts, cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, args.split_index)?;

    let policy_hash =
        compute_instruction_hash(&args.policy_data, policy_accounts, policy_program_key)?;
    let cpi_hash = compute_instruction_hash(&args.cpi_data, cpi_accounts, cpi_program_key)?;
    let expected_message_hash =
        compute_execute_message_hash(last_nonce, args.timestamp, policy_hash, cpi_hash)?;
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        args.passkey_public_key,
        args.signature,
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;

    let policy_signer = get_policy_signer(smart_wallet_key, wallet_device_key, credential_hash)?;
    let policy_data = &args.policy_data;
    require!(
        policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidInstructionDiscriminator
    );
    execute_cpi(
        policy_accounts,
        policy_data,
        &ctx.accounts.policy_program,
        &policy_signer,
    )?;

    let wallet_signer = PdaSigner {
        seeds: vec![SMART_WALLET_SEED.to_vec(), wallet_id.to_le_bytes().to_vec()],
        bump: wallet_bump,
    };
    execute_cpi(
        cpi_accounts,
        &args.cpi_data,
        &ctx.accounts.cpi_program,
        &wallet_signer,
    )?;

    ctx.accounts.wallet_state.last_nonce = validation::safe_increment_nonce(last_nonce);
    Ok(())
}

#[derive(Accounts)]
#[instruction(args: ExecuteArgs)]
pub struct Execute<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, wallet_state.wallet_id.to_le_bytes().as_ref()],
        bump = wallet_state.bump,
    )]
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
        executable,
        address = wallet_state.policy_program
    )]
    /// CHECK: Validated to be executable and match wallet_state.policy_program
    pub policy_program: UncheckedAccount<'info>,

    #[account(executable)]
    /// CHECK: Validated to be executable
    pub cpi_program: UncheckedAccount<'info>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: Instruction sysvar
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
