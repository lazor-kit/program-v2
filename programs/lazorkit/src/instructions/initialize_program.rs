use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, PolicyProgramRegistry},
};

/// Initialize the LazorKit program with essential configuration
///
/// Sets up the program's initial state including the policy program registry
/// and default configuration parameters. This must be called before any
/// other operations can be performed.
pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
    // Step 1: Validate the default policy program
    // Ensure the provided policy program is executable (not a data account)
    if !ctx.accounts.default_policy_program.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }

    // Step 2: Initialize the policy program registry
    // Register the default policy program as the first approved program
    let policy_program_registry = &mut ctx.accounts.policy_program_registry;
    policy_program_registry.registered_programs = vec![ctx.accounts.default_policy_program.key()];

    // Step 3: Initialize the program configuration
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.signer.key();
    config.fee_payer_fee = crate::constants::DEFAULT_FEE_PAYER_FEE;
    config.referral_fee = crate::constants::DEFAULT_REFERRAL_FEE;
    config.lazorkit_fee = crate::constants::DEFAULT_LAZORKIT_FEE;
    config.default_policy_program_id = ctx.accounts.default_policy_program.key();
    config.is_paused = false;

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    /// The signer of the transaction, who will be the initial authority.
    #[account(mut)]
    pub signer: Signer<'info>,

    /// The program's configuration account.
    #[account(
        init,
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
        bump,
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    /// The default policy program to be used for new smart wallets.
    /// CHECK: This is checked to be executable.
    pub default_policy_program: UncheckedAccount<'info>,

    /// The system program.
    pub system_program: Program<'info, System>,
}
