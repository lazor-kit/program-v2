use anchor_lang::prelude::*;

use crate::{error::LazorKitError, state::Config};

pub fn withdraw_vault(ctx: Context<WithdrawVault>, amount: u64) -> Result<()> {
    let vault_info = &ctx.accounts.vault.to_account_info();

    // Withdraw SOL from vault to destination
    crate::state::LazorKitVault::remove_sol(
        vault_info,
        &ctx.accounts.destination.to_account_info(),
        amount,
    )?;

    msg!("Withdrew {} lamports from vault", amount);

    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawVault<'info> {
    /// The current authority of the program.
    #[account(
        mut,
        constraint = authority.key() == config.authority @ LazorKitError::AuthorityMismatch
    )]
    pub authority: Signer<'info>,

    /// The program's configuration account.
    #[account(
        seeds = [Config::PREFIX_SEED],
        bump,
        has_one = authority @ LazorKitError::InvalidAuthority
    )]
    pub config: Box<Account<'info, Config>>,

    /// Individual vault PDA (empty account that holds SOL)
    #[account(mut)]
    /// CHECK: Empty PDA vault that only holds SOL
    pub vault: UncheckedAccount<'info>,

    /// CHECK: Destination account (where funds go)
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,

    /// System program
    pub system_program: Program<'info, System>,
}
