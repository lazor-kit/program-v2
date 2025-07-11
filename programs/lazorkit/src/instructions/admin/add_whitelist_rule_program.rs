use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, WhitelistRulePrograms},
};

pub fn add_whitelist_rule_program(ctx: Context<AddWhitelistRuleProgram>) -> Result<()> {
    let program_info = ctx
        .remaining_accounts
        .first()
        .ok_or(LazorKitError::InvalidRemainingAccounts)?;

    if !program_info.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }

    let whitelist = &mut ctx.accounts.whitelist_rule_programs;
    let program_id = program_info.key();

    if whitelist.list.contains(&program_id) {
        // The program is already in the whitelist, so we can just return Ok.
        // Or we can return an error, e.g., ProgramAlreadyWhitelisted.
        // For an "upsert" or "add" operation, returning Ok is idempotent and often preferred.
        return Ok(());
    }

    if whitelist.list.len() >= whitelist.list.capacity() {
        return err!(LazorKitError::WhitelistFull);
    }

    whitelist.list.push(program_id);

    Ok(())
}

#[derive(Accounts)]
pub struct AddWhitelistRuleProgram<'info> {
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
        seeds = [WhitelistRulePrograms::PREFIX_SEED],
        bump,
    )]
    pub whitelist_rule_programs: Account<'info, WhitelistRulePrograms>,
} 