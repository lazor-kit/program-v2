use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::system_instruction;

use crate::constants::SMART_WALLET_SEED;
use crate::error::LazorKitError;
use crate::instructions::AddDeviceArgs;
use crate::security::validation;
use crate::state::{WalletDevice, WalletState};
use crate::utils::{
    compute_add_device_message_hash, compute_device_hash, compute_instruction_hash,
    create_wallet_device_hash, execute_cpi, get_policy_signer, sighash, verify_authorization_hash,
};
use crate::ID;

pub fn add_device<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, AddDevice<'info>>,
    args: AddDeviceArgs,
) -> Result<()> {
    let policy_hash = compute_instruction_hash(
        &args.policy_data,
        ctx.remaining_accounts,
        ctx.accounts.policy_program.key(),
    )?;

    let new_device_hash = compute_device_hash(
        args.new_device_passkey_public_key,
        args.new_device_credential_hash,
    );

    let expected_message_hash = compute_add_device_message_hash(
        ctx.accounts.wallet_state.last_nonce,
        args.timestamp,
        policy_hash,
        new_device_hash,
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

    require!(
        args.policy_data.get(0..8) == Some(&sighash("global", "add_device")),
        LazorKitError::InvalidInstructionDiscriminator
    );

    // create the new wallet device
    let new_wallet_device = &mut ctx.accounts.new_wallet_device;
    new_wallet_device.set_inner(WalletDevice {
        bump: ctx.bumps.new_wallet_device,
        passkey_pubkey: args.new_device_passkey_public_key,
        credential_hash: args.new_device_credential_hash,
        smart_wallet: ctx.accounts.smart_wallet.key(),
    });

    let policy_signer = get_policy_signer(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.key(),
        ctx.accounts.wallet_device.credential_hash,
    )?;
    let policy_data = execute_cpi(
        ctx.remaining_accounts,
        &args.policy_data,
        &ctx.accounts.policy_program,
        policy_signer,
    )?;

    // Update the policy data size
    let diff_bytes = policy_data.len() - ctx.accounts.wallet_state.policy_data.len();
    let new_size = ctx.accounts.wallet_state.to_account_info().data_len() + diff_bytes;
    let rent = Rent::get()?;
    let new_minimum_balance = rent.minimum_balance(new_size);
    let lamports_diff =
        new_minimum_balance.saturating_sub(ctx.accounts.wallet_state.to_account_info().lamports());
    invoke(
        &system_instruction::transfer(
            ctx.accounts.payer.key,
            ctx.accounts.wallet_state.to_account_info().key,
            lamports_diff,
        ),
        &[
            ctx.accounts.payer.to_account_info().clone(),
            ctx.accounts.wallet_state.to_account_info().clone(),
            ctx.accounts.system_program.to_account_info().clone(),
        ],
    )?;

    ctx.accounts
        .wallet_state
        .to_account_info()
        .realloc(new_size, true)?;

    // Update the nonce
    ctx.accounts.wallet_state.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.wallet_state.last_nonce);

    // Update the policy data
    ctx.accounts.wallet_state.policy_data = policy_data;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: AddDeviceArgs)]
pub struct AddDevice<'info> {
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
        init,
        payer = payer,
        space = 8 + WalletDevice::INIT_SPACE,
        seeds = [WalletDevice::PREFIX_SEED, &create_wallet_device_hash(smart_wallet.key(), args.new_device_credential_hash)],
        bump
    )]
    pub new_wallet_device: Box<Account<'info, WalletDevice>>,

    /// CHECK: executable policy program
    #[account(
        address = wallet_state.policy_program
    )]
    pub policy_program: UncheckedAccount<'info>,

    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
