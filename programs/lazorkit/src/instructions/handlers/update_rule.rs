use super::super::{Execute, ExecuteArgs};
use crate::error::LazorKitError;
use crate::state::Message;
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer, sighash, split_remaining_accounts,
};
use anchor_lang::prelude::*;

/// Handle `Action::UpdateRuleProgram`
pub fn handle<'c: 'info, 'info>(
    ctx: &mut Context<'_, '_, 'c, 'info, Execute<'info>>,
    args: &ExecuteArgs,
    msg: &Message,
) -> Result<()> {
    let old_rule_program = &ctx.accounts.authenticator_program;
    let new_rule_program = &ctx.accounts.cpi_program;

    // --- executable checks
    if !old_rule_program.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }
    if !new_rule_program.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }

    // --- whitelist checks
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &old_rule_program.key(),
    )?;
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &new_rule_program.key(),
    )?;

    // --- destroy / init discriminator check
    require!(
        msg.rule_data.get(0..8) == Some(&sighash("global", "destroy")),
        LazorKitError::InvalidDestroyDiscriminator
    );
    require!(
        msg.cpi_data.get(0..8) == Some(&sighash("global", "init_rule")),
        LazorKitError::InvalidInitRuleDiscriminator
    );

    // --- program difference & default rule constraints
    require!(
        old_rule_program.key() != new_rule_program.key(),
        LazorKitError::RuleProgramsIdentical
    );
    // This constraint means that a user can only switch between the default rule
    // and another rule. They cannot switch between two non-default rules.
    let default_rule_program = ctx.accounts.config.default_rule_program;
    require!(
        old_rule_program.key() == default_rule_program
            || new_rule_program.key() == default_rule_program,
        LazorKitError::NoDefaultRuleProgram
    );

    // --- update config
    ctx.accounts.smart_wallet_config.rule_program = new_rule_program.key();

    // --- signer & account slices
    let rule_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.bumps.smart_wallet_authenticator,
    );
    let (rule_accounts, cpi_accounts) =
        split_remaining_accounts(ctx.remaining_accounts, msg.split_index)?;

    // --- destroy old rule instance
    execute_cpi(
        rule_accounts,
        &msg.rule_data,
        old_rule_program,
        Some(rule_signer.clone()),
    )?;

    // --- init new rule instance
    execute_cpi(
        cpi_accounts,
        &msg.cpi_data,
        new_rule_program,
        Some(rule_signer),
    )?;

    Ok(())
}
