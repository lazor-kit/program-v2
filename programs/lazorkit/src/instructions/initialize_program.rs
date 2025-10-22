use anchor_lang::prelude::*;

use crate::constants::{
    DEFAULT_CREATE_WALLET_FEE, DEFAULT_FEE_PAYER_FEE, DEFAULT_LAZORKIT_FEE, DEFAULT_REFERRAL_FEE,
};
use crate::state::{Config, PolicyProgramRegistry};

pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
    let policy_program_registry = &mut ctx.accounts.policy_program_registry;
    policy_program_registry.registered_programs = vec![ctx.accounts.default_policy_program.key()];

    let config = &mut ctx.accounts.config;
    config.set_inner(Config {
        authority: ctx.accounts.signer.key(),
        create_smart_wallet_fee: DEFAULT_CREATE_WALLET_FEE,
        fee_payer_fee: DEFAULT_FEE_PAYER_FEE,
        referral_fee: DEFAULT_REFERRAL_FEE,
        lazorkit_fee: DEFAULT_LAZORKIT_FEE,
        default_policy_program_id: ctx.accounts.default_policy_program.key(),
        is_paused: false,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + Config::INIT_SPACE,
        seeds = [Config::PREFIX_SEED],
        bump,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        init,
        payer = signer,
        space = 8 + PolicyProgramRegistry::INIT_SPACE,
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    #[account(executable)]
    /// CHECK: This is checked to be executable.
    pub default_policy_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
