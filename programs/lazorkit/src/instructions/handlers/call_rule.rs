use super::super::{Execute, ExecuteArgs};
use crate::error::LazorKitError;
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

    // 1. Executable and whitelist check
    if !rule_program.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }
    check_whitelist(&ctx.accounts.whitelist_rule_programs, &rule_program.key())?;

    // // 2. Optionally create a new authenticator
    // if let Some(new_passkey) = args.create_new_authenticator {
    //     let new_smart_wallet_authenticator = &ctx
    //         .remaining_accounts
    //         .first()
    //         .ok_or(LazorKitError::InvalidRemainingAccounts)?;

    //     SmartWalletAuthenticator::init(
    //         &new_smart_wallet_authenticator,
    //         ctx.accounts.payer.to_account_info(),
    //         ctx.accounts.system_program.to_account_info(),
    //         ctx.accounts.smart_wallet.key(),
    //         new_passkey,
    //         Vec::new(),
    //     )?;
    // }

    // 3. signer & account slice
    let rule_signer: crate::utils::PdaSigner = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.bumps.smart_wallet_authenticator,
    );
    let (rule_accounts, _cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, msg.split_index)?;

    execute_cpi(
        rule_accounts,
        &msg.rule_data,
        rule_program,
        Some(rule_signer),
    )?;
    Ok(())
}
