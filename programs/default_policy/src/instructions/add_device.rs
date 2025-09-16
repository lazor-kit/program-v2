use crate::{state::Policy, ID};
use anchor_lang::prelude::*;
use lazorkit::{program::Lazorkit, state::WalletDevice};

pub fn add_device(ctx: Context<AddDevice>) -> Result<()> {
    let new_policy = &mut ctx.accounts.new_policy;

    new_policy.smart_wallet = ctx.accounts.policy.smart_wallet.key();
    new_policy.wallet_device = ctx.accounts.new_wallet_device.key();

    Ok(())
}

#[derive(Accounts)]
pub struct AddDevice<'info> {
    #[account(mut)]
    pub smart_wallet: Signer<'info>,

    #[account(
        owner = lazorkit.key(),
        signer,
    )]
    pub wallet_device: Account<'info, WalletDevice>,

    /// CHECK:
    #[account(mut)]
    pub new_wallet_device: UncheckedAccount<'info>,

    #[account(
        seeds = [Policy::PREFIX_SEED, wallet_device.key().as_ref()],
        bump,
        owner = ID,
        constraint = policy.wallet_device == wallet_device.key(),
    )]
    pub policy: Account<'info, Policy>,

    #[account(
        init,
        payer = smart_wallet,
        space = 8 + Policy::INIT_SPACE,
        seeds = [Policy::PREFIX_SEED, new_wallet_device.key().as_ref()],
        bump,
    )]
    pub new_policy: Account<'info, Policy>,

    pub lazorkit: Program<'info, Lazorkit>,

    pub system_program: Program<'info, System>,
}
