use crate::state::Policy;
use anchor_lang::prelude::*;
use lazorkit::program::Lazorkit;

pub fn init_policy(ctx: Context<InitPolicy>) -> Result<()> {
    let policy = &mut ctx.accounts.policy;

    policy.smart_wallet = ctx.accounts.smart_wallet.key();
    policy.wallet_device = ctx.accounts.wallet_device.key();

    Ok(())
}

#[derive(Accounts)]
pub struct InitPolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK:
    pub smart_wallet: UncheckedAccount<'info>,

    /// CHECK:
    #[account(mut, signer)]
    pub wallet_device: UncheckedAccount<'info>,

    #[account(
        init,
        payer = payer,
        space = 8 + Policy::INIT_SPACE,
        seeds = [Policy::PREFIX_SEED, wallet_device.key().as_ref()],
        bump,
    )]
    pub policy: Account<'info, Policy>,

    pub lazorkit: Program<'info, Lazorkit>,

    pub system_program: Program<'info, System>,
}
