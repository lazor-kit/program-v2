use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, PolicyProgramRegistry},
};

pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    // Check if the default policy program is executable
    if !ctx.accounts.default_policy_program.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }

    let policy_program_registry = &mut ctx.accounts.policy_program_registry;
    policy_program_registry.programs = vec![ctx.accounts.default_policy_program.key()];

    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.signer.key();
    config.fee_payer_fee = 30000; // LAMPORTS
    config.referral_fee = 10000; // LAMPORTS
    config.lazorkit_fee = 10000; // LAMPORTS
    config.default_policy_program = ctx.accounts.default_policy_program.key();
    config.is_paused = false;

    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// The signer of the transaction, who will be the initial authority.
    #[account(mut)]
    pub signer: Signer<'info>,

    /// The program's configuration account.
    #[account(
        init_if_needed,
        payer = signer,
        space = 8 + Config::INIT_SPACE,
        seeds = [Config::PREFIX_SEED],
        bump,
    )]
    pub config: Box<Account<'info, Config>>,

    /// The registry of policy programs that can be used with smart wallets.
    #[account(
        init,
        payer = signer,
        space = 8 + PolicyProgramRegistry::INIT_SPACE,
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    /// The default policy program to be used for new smart wallets.
    /// CHECK: This is checked to be executable.
    pub default_policy_program: UncheckedAccount<'info>,

    /// The system program.
    pub system_program: Program<'info, System>,
}
