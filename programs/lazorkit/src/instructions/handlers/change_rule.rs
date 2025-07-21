use super::super::{Execute, ExecuteArgs};
use crate::error::LazorKitError;
use crate::security::validation;
use crate::state::Message;
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer, sighash, split_remaining_accounts,
};
use anchor_lang::prelude::*;

/// Handle `Action::ChangeRuleProgram`
pub fn handle<'c: 'info, 'info>(
    ctx: &mut Context<'_, '_, 'c, 'info, Execute<'info>>,
    args: &ExecuteArgs,
    msg: &Message,
) -> Result<()> {
    let old_rule_program = &ctx.accounts.authenticator_program;
    let new_rule_program = &ctx.accounts.cpi_program;

    // === Validate both programs are executable ===
    validation::validate_program_executable(old_rule_program)?;
    validation::validate_program_executable(new_rule_program)?;

    // === Verify both programs are whitelisted ===
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &old_rule_program.key(),
    )?;
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &new_rule_program.key(),
    )?;

    // === Validate current rule program matches wallet config ===
    require!(
        old_rule_program.key() == ctx.accounts.smart_wallet_config.rule_program,
        LazorKitError::InvalidProgramAddress
    );

    // === Check if rule_data is provided and verify destroy discriminator ===
    let rule_data = msg
        .rule_data
        .as_ref()
        .ok_or(LazorKitError::RuleDataRequired)?;
    
    // Validate rule data size
    validation::validate_rule_data(rule_data)?;
    
    require!(
        rule_data.get(0..8) == Some(&sighash("global", "destroy")),
        LazorKitError::InvalidDestroyDiscriminator
    );
    
    // === Validate init_rule discriminator ===
    require!(
        msg.cpi_data.get(0..8) == Some(&sighash("global", "init_rule")),
        LazorKitError::InvalidInitRuleDiscriminator
    );

    // === Ensure programs are different ===
    require!(
        old_rule_program.key() != new_rule_program.key(),
        LazorKitError::RuleProgramsIdentical
    );
    
    // === Default rule constraint ===
    // This constraint means that a user can only switch between the default rule
    // and another rule. They cannot switch between two non-default rules.
    let default_rule_program = ctx.accounts.config.default_rule_program;
    require!(
        old_rule_program.key() == default_rule_program
            || new_rule_program.key() == default_rule_program,
        LazorKitError::NoDefaultRuleProgram
    );

    // === Update wallet configuration ===
    msg!("Changing rule program from {} to {}", 
        old_rule_program.key(), 
        new_rule_program.key()
    );
    
    ctx.accounts.smart_wallet_config.rule_program = new_rule_program.key();

    // === Create PDA signer ===
    let rule_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.bump,
    );
    
    // === Split and validate accounts ===
    let (rule_accounts, cpi_accounts) =
        split_remaining_accounts(ctx.remaining_accounts, msg.split_index)?;
    
    // Ensure we have sufficient accounts for both operations
    require!(
        !rule_accounts.is_empty(),
        LazorKitError::InsufficientRuleAccounts
    );
    require!(
        !cpi_accounts.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );

    // === Destroy old rule instance ===
    msg!("Destroying old rule instance");
    
    execute_cpi(
        rule_accounts,
        rule_data,
        old_rule_program,
        Some(rule_signer.clone()),
    )?;

    // === Initialize new rule instance ===
    msg!("Initializing new rule instance");
    
    execute_cpi(
        cpi_accounts,
        &msg.cpi_data,
        new_rule_program,
        Some(rule_signer),
    )?;

    msg!("Rule program changed successfully");
    
    Ok(())
}
