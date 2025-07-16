use anchor_lang::prelude::*;

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
    // 1. Whitelist & executable check for the rule program
    let rule_program_info = &ctx.accounts.authenticator_program;
    if !rule_program_info.executable {
        return err!(LazorKitError::ProgramNotExecutable);
    }
    check_whitelist(
        &ctx.accounts.whitelist_rule_programs,
        &rule_program_info.key(),
    )?;

    // 2. Prepare PDA signer for rule CPI
    let rule_signer = get_pda_signer(
        &_args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.bumps.smart_wallet_authenticator,
    );

    let (rule_accounts, cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, msg.split_index)?;

    // 3. Verify rule discriminator
    require!(
        msg.rule_data.get(0..8) == Some(&sighash("global", "check_rule")),
        LazorKitError::InvalidCheckRuleDiscriminator
    );

    // 4. Execute rule CPI to check if the transaction is allowed
    execute_cpi(
        rule_accounts,
        &msg.rule_data,
        rule_program_info,
        Some(rule_signer),
    )?;

    // 5. Execute main CPI or transfer lamports
    if msg.cpi_data.get(0..4) == Some(&SOL_TRANSFER_DISCRIMINATOR)
        && ctx.accounts.cpi_program.key() == anchor_lang::solana_program::system_program::ID
    {
        // This is a native SOL transfer
        require!(
            !cpi_accounts.is_empty(),
            LazorKitError::SolTransferInsufficientAccounts
        );

        let amount_bytes = msg
            .cpi_data
            .get(4..12)
            .ok_or(LazorKitError::InvalidCpiData)?;
        let amount = u64::from_le_bytes(
            amount_bytes
                .try_into()
                .map_err(|_| LazorKitError::InvalidCpiData)?,
        );

        let destination_account = &cpi_accounts[1];

        transfer_sol_from_pda(&ctx.accounts.smart_wallet, destination_account, amount)?;
    } else {
        // This is a general CPI
        let wallet_signer = PdaSigner {
            seeds: vec![
                SMART_WALLET_SEED.to_vec(),
                ctx.accounts.smart_wallet_config.id.to_le_bytes().to_vec(),
            ],
            bump: ctx.accounts.smart_wallet_config.bump,
        };
        execute_cpi(
            cpi_accounts,
            &msg.cpi_data,
            &ctx.accounts.cpi_program,
            Some(wallet_signer),
        )?;
    }

    Ok(())
}
