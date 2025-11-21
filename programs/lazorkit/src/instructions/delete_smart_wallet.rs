use std::vec;

use anchor_lang::prelude::*;

const ADMIN_PUBLIC_KEY: Pubkey = pubkey!("BE8duRBDmh4cF4Ecz4TBCNgNAMCaonrpQiEiQ1xfQmab");

use crate::{
    error::LazorKitError,
    state::{WalletDevice, WalletState},
    utils::transfer_sol_util,
    ID,
};

pub fn delete_smart_wallet(ctx: Context<DeleteSmartWallet>) -> Result<()> {
    // transfer lamports to the admin
    transfer_sol_util(
        &ctx.accounts.smart_wallet.to_account_info(),
        ctx.accounts.wallet_state.wallet_id,
        ctx.accounts.wallet_state.bump,
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        0,
    )?;
    Ok(())
}

#[derive(Accounts)]
#[instruction()]
pub struct DeleteSmartWallet<'info> {
    #[account(
        mut,
        address = ADMIN_PUBLIC_KEY @ LazorKitError::UnauthorizedAdmin
    )]
    pub payer: Signer<'info>,

    #[account(mut)]
    /// CHECK:
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
        close = payer,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        mut,
        owner = ID,
        close = payer,
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    pub system_program: Program<'info, System>,
}
