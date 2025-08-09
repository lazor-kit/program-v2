use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, WhitelistRulePrograms},
};

pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    // Check if the default rule program is executable
    if !ctx.accounts.default_rule_program.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }

    let whitelist_rule_programs = &mut ctx.accounts.whitelist_rule_programs;
    whitelist_rule_programs.list = vec![ctx.accounts.default_rule_program.key()];

    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.signer.key();
    config.create_smart_wallet_fee = 0; // LAMPORTS
    config.execute_fee = 0; // LAMPORTS
    config.default_rule_program = ctx.accounts.default_rule_program.key();
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

    /// The list of whitelisted rule programs that can be used with smart wallets.
    #[account(
        init_if_needed,
        payer = signer,
        space = 8 + WhitelistRulePrograms::INIT_SPACE,
        seeds = [WhitelistRulePrograms::PREFIX_SEED],
        bump
    )]
    pub whitelist_rule_programs: Box<Account<'info, WhitelistRulePrograms>>,

    /// The default rule program to be used for new smart wallets.
    /// CHECK: This is checked to be executable.
    pub default_rule_program: UncheckedAccount<'info>,

    /// The system program.
    pub system_program: Program<'info, System>,
}
