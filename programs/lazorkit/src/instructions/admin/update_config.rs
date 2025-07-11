use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, UpdateConfigType},
};

pub fn update_config(
    ctx: Context<UpdateConfig>,
    param: UpdateConfigType,
    value: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    match param {
        UpdateConfigType::CreateWalletFee => {
            config.create_smart_wallet_fee = value;
        }
        UpdateConfigType::ExecuteFee => {
            config.execute_fee = value;
        }
        UpdateConfigType::DefaultRuleProgram => {
            let new_default_rule_program_info = ctx
                .remaining_accounts
                .first()
                .ok_or(LazorKitError::InvalidRemainingAccounts)?;

            // Check if the new default rule program is executable
            if !new_default_rule_program_info.executable {
                return err!(LazorKitError::ProgramNotExecutable);
            }
            config.default_rule_program = new_default_rule_program_info.key();
        }
        UpdateConfigType::Admin => {
            let new_admin_info = ctx
                .remaining_accounts
                .first()
                .ok_or(LazorKitError::InvalidRemainingAccounts)?;
            config.authority = new_admin_info.key();
        }
    }
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    /// The current authority of the program.
    #[account(mut)]
    pub authority: Signer<'info>,

    /// The program's configuration account.
    #[account(
        mut,
        seeds = [Config::PREFIX_SEED],
        bump,
        has_one = authority
    )]
    pub config: Box<Account<'info, Config>>,
}
