use anchor_lang::prelude::*;

use crate::security::validation;
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer, sighash, split_remaining_accounts,
    transfer_sol_from_pda, PdaSigner,
};
use crate::{
    constants::{SMART_WALLET_SEED, SOL_TRANSFER_DISCRIMINATOR},
    error::LazorKitError,
};

use super::super::{Execute, ExecuteArgs};
use crate::state::Message;

/// Handle `Action::ExecuteTx`
pub fn handle<'c: 'info, 'info>(
    ctx: &mut Context<'_, '_, 'c, 'info, Execute<'info>>,
    _args: &ExecuteArgs,
    msg: &Message,
) -> Result<()> {
    // 1. Validate and check rule program
    let rule_program_info = &ctx.accounts.authenticator_program;
    
    // Ensure rule program is executable
    validation::validate_program_executable(rule_program_info)?;
    
    // Verify rule program is whitelisted
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &rule_program_info.key(),
    )?;
    
    // Ensure rule program matches wallet configuration
    require!(
        rule_program_info.key() == ctx.accounts.smart_wallet_config.rule_program,
        LazorKitError::InvalidProgramAddress
    );

    // 2. Prepare PDA signer for rule CPI
    let rule_signer = get_pda_signer(
        &_args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.bump,
    );

    // 3. Split remaining accounts
    let (rule_accounts, cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, msg.split_index)?;
    
    // Validate account counts
    require!(
        !rule_accounts.is_empty(),
        LazorKitError::InsufficientRuleAccounts
    );

    // 4. Check if rule_data is provided and verify rule discriminator
    let rule_data = msg.rule_data.as_ref().ok_or(LazorKitError::RuleDataRequired)?;
    require!(
        rule_data.get(0..8) == Some(&sighash("global", "check_rule")),
        LazorKitError::InvalidCheckRuleDiscriminator
    );

    // 5. Execute rule CPI to check if the transaction is allowed
    msg!("Executing rule check for smart wallet: {}", ctx.accounts.smart_wallet.key());
    
    execute_cpi(
        rule_accounts,
        rule_data,
        rule_program_info,
        Some(rule_signer),
    )?;
    
    msg!("Rule check passed");

    // 6. Execute main CPI or transfer lamports (inline data)
    if msg.cpi_data.get(0..4) == Some(&SOL_TRANSFER_DISCRIMINATOR)
        && ctx.accounts.cpi_program.key() == anchor_lang::solana_program::system_program::ID
    {
        // === Native SOL Transfer ===
        require!(
            cpi_accounts.len() >= 2,
            LazorKitError::SolTransferInsufficientAccounts
        );

        // Extract and validate amount
        let amount_bytes = msg
            .cpi_data
            .get(4..12)
            .ok_or(LazorKitError::InvalidCpiData)?;
        let amount = u64::from_le_bytes(
            amount_bytes
                .try_into()
                .map_err(|_| LazorKitError::InvalidCpiData)?,
        );
        
        // Validate amount
        validation::validate_lamport_amount(amount)?;

        // Ensure destination is valid
        let destination_account = &cpi_accounts[1];
        require!(
            destination_account.key() != ctx.accounts.smart_wallet.key(),
            LazorKitError::InvalidAccountData
        );
        
        // Check wallet has sufficient balance
        let wallet_balance = ctx.accounts.smart_wallet.lamports();
        let rent_exempt = Rent::get()?.minimum_balance(0);
        let total_needed = amount
            .checked_add(ctx.accounts.config.execute_fee)
            .ok_or(LazorKitError::IntegerOverflow)?
            .checked_add(rent_exempt)
            .ok_or(LazorKitError::IntegerOverflow)?;
        
        require!(
            wallet_balance >= total_needed,
            LazorKitError::InsufficientLamports
        );

        msg!("Transferring {} lamports to {}", amount, destination_account.key());
        
        transfer_sol_from_pda(&ctx.accounts.smart_wallet, destination_account, amount)?;
    } else {
        // === General CPI ===
        
        // Validate CPI program
        validation::validate_program_executable(&ctx.accounts.cpi_program)?;
        
        // Ensure CPI program is not this program (prevent reentrancy)
        require!(
            ctx.accounts.cpi_program.key() != crate::ID,
            LazorKitError::ReentrancyDetected
        );
        
        // Ensure sufficient accounts for CPI
        require!(
            !cpi_accounts.is_empty(),
            LazorKitError::InsufficientCpiAccounts
        );
        
        // Create wallet signer
        let wallet_signer = PdaSigner {
            seeds: vec![
                SMART_WALLET_SEED.to_vec(),
                ctx.accounts.smart_wallet_config.id.to_le_bytes().to_vec(),
            ],
            bump: ctx.accounts.smart_wallet_config.bump,
        };
        
        msg!("Executing CPI to program: {}", ctx.accounts.cpi_program.key());
        
        execute_cpi(cpi_accounts, &msg.cpi_data, &ctx.accounts.cpi_program, Some(wallet_signer))?;
    }

    msg!("Transaction executed successfully");

    Ok(())
}
