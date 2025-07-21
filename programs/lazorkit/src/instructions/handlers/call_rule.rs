use super::super::{Execute, ExecuteArgs};
use crate::error::LazorKitError;
use crate::security::validation;
use crate::state::{Message, SmartWalletAuthenticator};
use crate::utils::{check_whitelist, execute_cpi, get_pda_signer, split_remaining_accounts};
use anchor_lang::prelude::*;

/// Handle `Action::CallRuleProgram` – may optionally create a new authenticator.
pub fn handle<'c: 'info, 'info>(
    ctx: &mut Context<'_, '_, 'c, 'info, Execute<'info>>,
    args: &ExecuteArgs,
    msg: &Message,
) -> Result<()> {
    let rule_program = &ctx.accounts.authenticator_program;

    // === Validate rule program ===
    validation::validate_program_executable(rule_program)?;
    check_whitelist(&ctx.accounts.whitelist_rule_programs, &rule_program.key())?;
    
    // Ensure rule program matches wallet configuration
    require!(
        rule_program.key() == ctx.accounts.smart_wallet_config.rule_program,
        LazorKitError::InvalidProgramAddress
    );

    // === Optionally create a new authenticator ===
    if let Some(new_passkey) = args.create_new_authenticator {
        msg!("Creating new authenticator for passkey");
        
        // Validate new passkey format
        require!(
            new_passkey[0] == 0x02 || new_passkey[0] == 0x03,
            LazorKitError::InvalidPasskeyFormat
        );
        
        // Get the new authenticator account from remaining accounts
        let new_smart_wallet_authenticator = ctx
            .remaining_accounts
            .first()
            .ok_or(LazorKitError::InvalidRemainingAccounts)?;
        
        // Ensure the account is not already initialized
        require!(
            new_smart_wallet_authenticator.data_is_empty(),
            LazorKitError::AccountAlreadyInitialized
        );
        
        // Initialize the new authenticator
        SmartWalletAuthenticator::init(
            &new_smart_wallet_authenticator,
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.smart_wallet.key(),
            new_passkey,
            Vec::new(), // Empty credential ID for secondary authenticators
        )?;
        
        msg!("New authenticator created: {}", new_smart_wallet_authenticator.key());
    }

    // === Prepare for rule CPI ===
    let rule_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.bump,
    );
    
    // Split accounts (skip first if new authenticator was created)
    let skip_count = if args.create_new_authenticator.is_some() { 1 } else { 0 };
    let remaining_for_split = &ctx.remaining_accounts[skip_count..];
    
    let (_, cpi_accounts) = split_remaining_accounts(remaining_for_split, msg.split_index)?;
    
    // Validate we have accounts for CPI
    require!(
        !cpi_accounts.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );
    
    // Validate CPI data
    validation::validate_cpi_data(&msg.cpi_data)?;
    
    msg!("Executing rule program CPI");
    
    execute_cpi(cpi_accounts, &msg.cpi_data, rule_program, Some(rule_signer))?;
    
    msg!("Rule program call completed successfully");
    
    Ok(())
}
