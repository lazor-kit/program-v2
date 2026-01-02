use crate::state::PolicyStruct;
use anchor_lang::prelude::*;

/// Initialize policy for a new smart wallet
pub fn init_policy(ctx: Context<InitPolicy>) -> Result<PolicyStruct> {
    Ok(PolicyStruct {
        smart_wallet: ctx.accounts.smart_wallet.key(),
        authoritis: vec![ctx.accounts.authority.key()],
    })
}

#[derive(Accounts)]
pub struct InitPolicy<'info> {
    pub authority: Signer<'info>,

    #[account(mut)]
    /// Must mut follow lazorkit standard
    pub smart_wallet: SystemAccount<'info>,
}
