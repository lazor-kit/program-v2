use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{PolicyProgramRegistry, Config},
};

/// Add a new policy program to the registry
///
/// Allows the program authority to register a new policy program in the
/// whitelist. Policy programs govern smart wallet transaction validation
/// and security rules. Only executable programs can be registered.
pub fn add_policy_program(ctx: Context<RegisterPolicyProgram>) -> Result<()> {
    let registry: &mut Account<'_, PolicyProgramRegistry> =
        &mut ctx.accounts.policy_program_registry;
    let program_id = ctx.accounts.new_policy_program.key();

    if registry.registered_programs.contains(&program_id) {
        return err!(LazorKitError::PolicyProgramAlreadyRegistered);
    }

    if registry.registered_programs.len() >= registry.registered_programs.capacity() {
        return err!(LazorKitError::WhitelistFull);
    }

    registry.registered_programs.push(program_id);

    Ok(())
}

#[derive(Accounts)]
pub struct RegisterPolicyProgram<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [Config::PREFIX_SEED],
        bump,
        has_one = authority
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
    )]
    pub policy_program_registry: Account<'info, PolicyProgramRegistry>,

    /// CHECK: executable policy program
    #[account(
        constraint = new_policy_program.executable @ LazorKitError::ProgramNotExecutable
    )]
    pub new_policy_program: UncheckedAccount<'info>,
}
