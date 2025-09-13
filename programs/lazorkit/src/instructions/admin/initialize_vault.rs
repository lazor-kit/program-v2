use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, LazorKitVault},
};

pub fn initialize_vault(_ctx: Context<InitializeVault>, index: u8) -> Result<()> {
    require!(
        index < LazorKitVault::MAX_VAULTS,
        LazorKitError::InvalidVaultIndex
    );

    // Vault is now just an empty PDA that holds SOL
    // No need to initialize any data - it's owned by the program and can hold lamports
    msg!(
        "Initialized empty PDA vault {} for LazorKit treasury",
        index
    );
    Ok(())
}

#[derive(Accounts)]
#[instruction(index: u8)]
pub struct InitializeVault<'info> {
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

    /// The empty vault PDA to initialize (just holds SOL, no data)
    #[account(
        init,
        payer = authority,
        space = 0, // Empty PDA - no data storage needed
        seeds = [LazorKitVault::PREFIX_SEED, &index.to_le_bytes()],
        bump
    )]
    /// CHECK: Empty PDA vault that only holds SOL
    pub vault: UncheckedAccount<'info>,

    /// System program
    pub system_program: Program<'info, System>,
}
