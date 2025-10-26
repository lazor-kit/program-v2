use anchor_lang::prelude::*;

use crate::constants::SMART_WALLET_SEED;
use crate::instructions::CallPolicyArgs;
use crate::security::validation;
use crate::state::{WalletDevice, WalletState};
use crate::utils::{
    compute_call_policy_message_hash, compute_instruction_hash, create_wallet_device_hash,
    execute_cpi, get_policy_signer, verify_authorization_hash,
};
use crate::ID;

pub fn call_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, CallPolicy<'info>>,
    args: CallPolicyArgs,
) -> Result<()> {
    let policy_hash = compute_instruction_hash(
        &args.policy_data,
        ctx.remaining_accounts,
        ctx.accounts.policy_program.key(),
    )?;
    let expected_message_hash = compute_call_policy_message_hash(
        ctx.accounts.wallet_state.last_nonce,
        args.timestamp,
        policy_hash,
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
    let policy_data = execute_cpi(
        ctx.remaining_accounts,
        &args.policy_data,
        &ctx.accounts.policy_program,
        policy_signer,
    )?;

    // Update the nonce
    ctx.accounts.wallet_state.last_nonce =
        validation::safe_increment_nonce(ctx.accounts.wallet_state.last_nonce);
    ctx.accounts.wallet_state.policy_data = policy_data;

    // Create the new wallet device account if it exists
    match args.new_wallet_device {
        Some(new_wallet_device_args) => {
            let new_wallet_device_account = &mut ctx.accounts.new_wallet_device.as_mut().unwrap();
            new_wallet_device_account.set_inner(WalletDevice {
                bump: ctx.bumps.new_wallet_device.unwrap(),
                passkey_pubkey: new_wallet_device_args.passkey_public_key,
                credential_hash: new_wallet_device_args.credential_hash,
                smart_wallet: ctx.accounts.smart_wallet.key(),
            });
        }
        _ => {}
    }

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CallPolicyArgs)]
pub struct CallPolicy<'info> {
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
        seeds = [WalletDevice::PREFIX_SEED, &create_wallet_device_hash(smart_wallet.key(), args.new_wallet_device.clone().unwrap().credential_hash)],
        bump
    )]
    pub new_wallet_device: Option<Box<Account<'info, WalletDevice>>>,

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
