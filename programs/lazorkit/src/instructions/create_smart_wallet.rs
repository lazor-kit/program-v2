use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

use crate::{
    constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
    error::LazorKitError,
    state::{WalletDevice, WalletState},
    utils::{create_wallet_device_hash, execute_cpi, get_policy_signer},
    ID,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateSmartWalletArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub credential_hash: [u8; 32],
    pub init_policy_data: Vec<u8>,
    pub wallet_id: u64,
    pub amount: u64,
    pub policy_data_size: u16,
}

/// Create a new smart wallet with passkey authentication
pub fn create_smart_wallet(
    ctx: Context<CreateSmartWallet>,
    args: CreateSmartWalletArgs,
) -> Result<()> {
    let smart_wallet_key = ctx.accounts.smart_wallet.key();
    let policy_program_key = ctx.accounts.policy_program.key();

    let policy_signer = get_policy_signer(
        smart_wallet_key,
        ctx.accounts.wallet_device.key(),
        args.credential_hash,
    )?;

    let policy_data = execute_cpi(
        ctx.remaining_accounts,
        &args.init_policy_data,
        &ctx.accounts.policy_program,
        &policy_signer,
    )?;

    require!(
        args.policy_data_size == policy_data.len() as u16,
        LazorKitError::InvalidPolicyDataSize
    );

    ctx.accounts.wallet_state.set_inner(WalletState {
        bump: ctx.bumps.smart_wallet,
        wallet_id: args.wallet_id,
        last_nonce: 0u64,
        policy_program: policy_program_key,
        policy_data,
    });

    ctx.accounts.wallet_device.set_inner(WalletDevice {
        bump: ctx.bumps.wallet_device,
        passkey_pubkey: args.passkey_public_key,
        credential_hash: args.credential_hash,
        smart_wallet: smart_wallet_key,
    });

    if args.amount > 0 {
        transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.smart_wallet.to_account_info(),
                },
            ),
            args.amount,
        )?;
    }

    require!(
        ctx.accounts.smart_wallet.lamports() >= crate::constants::EMPTY_PDA_RENT_EXEMPT_BALANCE,
        LazorKitError::InsufficientBalanceForFee
    );

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CreateSmartWalletArgs)]
pub struct CreateSmartWallet<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, args.wallet_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        init,
        payer = payer,
        space = WalletState::DISCRIMINATOR.len() + WalletState::INIT_SPACE + args.policy_data_size as usize,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        init,
        payer = payer,
        space = WalletDevice::DISCRIMINATOR.len() + WalletDevice::INIT_SPACE,
        seeds = [WalletDevice::PREFIX_SEED, &create_wallet_device_hash(smart_wallet.key(), args.credential_hash)],
        bump
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    #[account(
        executable,
        constraint = policy_program.key() != ID @ LazorKitError::ReentrancyDetected
    )]
    /// CHECK: Validated to be executable and not self-reentrancy
    pub policy_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
