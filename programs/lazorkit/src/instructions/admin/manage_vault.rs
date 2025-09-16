use anchor_lang::prelude::*;

use crate::{error::LazorKitError, state::{LazorKitVault, ProgramConfig}};

pub fn manage_vault(ctx: Context<ManageVault>, action: u8, amount: u64, index: u8) -> Result<()> {

    LazorKitVault::validate_vault_for_index(&ctx.accounts.vault.key(), index, &crate::ID)?;

     match action {
        0 => {
            crate::state::LazorKitVault::add_sol(&ctx.accounts.vault, &ctx.accounts.destination, &ctx.accounts.system_program, amount)?
        }
        1 => {
            crate::state::LazorKitVault::remove_sol(&ctx.accounts.vault, &ctx.accounts.destination, &ctx.accounts.system_program, amount, index, ctx.bumps.vault)?
        }
        _ => {
            return Err(LazorKitError::InvalidAction.into());
        }
     }

    Ok(())
}

#[derive(Accounts)]
#[instruction(action: u8, amount: u64, index: u8)]
pub struct ManageVault<'info> {
    /// The current authority of the program.
    #[account(
        mut,
        constraint = authority.key() == config.authority @ LazorKitError::AuthorityMismatch
    )]
    pub authority: Signer<'info>,

    /// The program's configuration account.
    #[account(
        seeds = [ProgramConfig::PREFIX_SEED],
        bump,
        has_one = authority @ LazorKitError::InvalidAuthority
    )]
    pub config: Box<Account<'info, ProgramConfig>>,

    /// Individual vault PDA (empty account that holds SOL)
    #[account(
        mut, 
        seeds = [LazorKitVault::PREFIX_SEED, &index.to_le_bytes()],
        bump,
    )]
    /// CHECK: Empty PDA vault that only holds SOL
    pub vault: SystemAccount<'info>,

    /// CHECK: Destination account (where funds go)
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,

    /// System program
    pub system_program: Program<'info, System>,
}
