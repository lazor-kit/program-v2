use anchor_lang::prelude::*;

use crate::{error::PolicyError, state::Policy, ID};

pub fn check_policy(_ctx: Context<CheckPolicy>) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct CheckPolicy<'info> {
    pub wallet_device: Signer<'info>,
    /// CHECK: bound via constraint to policy.smart_wallet
    pub smart_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        owner = ID,
        constraint = wallet_device.key() == policy.wallet_device @ PolicyError::UnAuthorize,
        constraint = policy.smart_wallet == smart_wallet.key() @ PolicyError::UnAuthorize,
    )]
    pub policy: Account<'info, Policy>,
}
