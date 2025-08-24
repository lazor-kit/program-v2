use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, PolicyProgramRegistry},
};

pub fn register_policy_program(ctx: Context<RegisterPolicyProgram>) -> Result<()> {
    let program_info = ctx
        .remaining_accounts
        .first()
        .ok_or(LazorKitError::InvalidRemainingAccounts)?;

    if !program_info.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }

    let registry = &mut ctx.accounts.policy_program_registry;
    let program_id = program_info.key();

    if registry.programs.contains(&program_id) {
        // The program is already in the whitelist, so we can just return Ok.
        // Or we can return an error, e.g., ProgramAlreadyWhitelisted.
        // For an "upsert" or "add" operation, returning Ok is idempotent and often preferred.
        return Ok(());
    }

    if registry.programs.len() >= registry.programs.capacity() {
        return err!(LazorKitError::WhitelistFull);
    }

    registry.programs.push(program_id);

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
}
