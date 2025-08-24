use anchor_lang::prelude::*;

use crate::{error::PolicyError, state::Policy, ID};

pub fn check_policy(_ctx: Context<CheckPolicy>) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct CheckPolicy<'info> {
    pub wallet_device: Signer<'info>,

    #[account(
        mut,
        owner = ID,
        constraint = wallet_device.key() == policy.wallet_device @ PolicyError::UnAuthorize,
    )]
    pub policy: Account<'info, Policy>,
}
