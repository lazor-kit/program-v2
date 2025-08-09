use anchor_lang::prelude::*;

use crate::instructions::{Args as _, ExecuteTxnArgs};
use crate::security::validation;
use crate::state::ExecuteMessage;
use crate::utils::{
    check_whitelist, execute_cpi, get_pda_signer, sighash, split_remaining_accounts,
    transfer_sol_from_pda, verify_authorization, PdaSigner,
};
use crate::{
    constants::{SMART_WALLET_SEED, SOL_TRANSFER_DISCRIMINATOR},
    error::LazorKitError,
};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn execute_txn_direct<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ExecuteTxn<'info>>,
    args: ExecuteTxnArgs,
) -> Result<()> {
    // 0. Validate args and global state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;

    // 0.1 Verify authorization and parse typed message
    let msg: ExecuteMessage = verify_authorization(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.smart_wallet_authenticator,
        ctx.accounts.smart_wallet.key(),
        args.passkey_pubkey,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        ctx.accounts.smart_wallet_config.last_nonce,
    )?;

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
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.bump,
    );

    // 3. Split remaining accounts
    let (rule_accounts, cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, args.split_index)?;

    // Validate account counts
    require!(
        !rule_accounts.is_empty(),
        LazorKitError::InsufficientRuleAccounts
    );

    // 4. Verify rule discriminator on provided rule_data
    let rule_data = &args.rule_data;
    require!(
        rule_data.get(0..8) == Some(&sighash("global", "check_rule")),
        LazorKitError::InvalidCheckRuleDiscriminator
    );

    // 4.1 Validate rule_data size and compare hash from message
    validation::validate_rule_data(rule_data)?;
    require!(
        hash(rule_data).to_bytes() == msg.rule_data_hash,
        LazorKitError::InvalidInstructionData
    );

    // 4.2 Compare rule accounts hash against message
    let mut rh = Hasher::default();
    rh.hash(rule_program_info.key.as_ref());
    for acc in rule_accounts.iter() {
        rh.hash(acc.key.as_ref());
        rh.hash(&[acc.is_writable as u8, acc.is_signer as u8]);
    }
    require!(
        rh.result().to_bytes() == msg.rule_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // 5. Execute rule CPI to check if the transaction is allowed
    msg!(
        "Executing rule check for smart wallet: {}",
        ctx.accounts.smart_wallet.key()
    );

    execute_cpi(
        rule_accounts,
        rule_data,
        rule_program_info,
        Some(rule_signer),
    )?;

    msg!("Rule check passed");

    // 6. Validate CPI payload and compare hashes
    validation::validate_cpi_data(&args.cpi_data)?;
    require!(
        hash(&args.cpi_data).to_bytes() == msg.cpi_data_hash,
        LazorKitError::InvalidInstructionData
    );
    let mut ch = Hasher::default();
    ch.hash(ctx.accounts.cpi_program.key.as_ref());
    for acc in cpi_accounts.iter() {
        ch.hash(acc.key.as_ref());
        ch.hash(&[acc.is_writable as u8, acc.is_signer as u8]);
    }
    require!(
        ch.result().to_bytes() == msg.cpi_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // 7. Execute main CPI or transfer lamports
    if args.cpi_data.get(0..4) == Some(&SOL_TRANSFER_DISCRIMINATOR)
        && ctx.accounts.cpi_program.key() == anchor_lang::solana_program::system_program::ID
    {
        // === Native SOL Transfer ===
        require!(
            cpi_accounts.len() >= 2,
            LazorKitError::SolTransferInsufficientAccounts
        );

        // Extract and validate amount
        let amount_bytes = args
            .cpi_data
            .get(4..12)
            .ok_or(LazorKitError::InvalidCpiData)?;
        let amount = u64::from_le_bytes(
            amount_bytes
                .try_into()
                .map_err(|_| LazorKitError::InvalidCpiData)?,
        );

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

        msg!(
            "Transferring {} lamports to {}",
            amount,
            destination_account.key()
        );
        transfer_sol_from_pda(&ctx.accounts.smart_wallet, destination_account, amount)?;
    } else {
        // === General CPI ===
        validation::validate_program_executable(&ctx.accounts.cpi_program)?;
        require!(
            ctx.accounts.cpi_program.key() != crate::ID,
            LazorKitError::ReentrancyDetected
        );
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

        msg!(
            "Executing CPI to program: {}",
            ctx.accounts.cpi_program.key()
        );
        execute_cpi(
            cpi_accounts,
            &args.cpi_data,
            &ctx.accounts.cpi_program,
            Some(wallet_signer),
        )?;
    }

    msg!("Transaction executed successfully");
    // 8. Increment nonce
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteTxn<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
        owner = crate::ID,
    )]
    /// CHECK: PDA verified by seeds
    pub smart_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [crate::state::SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = crate::ID,
    )]
    pub smart_wallet_config: Box<Account<'info, crate::state::SmartWalletConfig>>,

    #[account(owner = crate::ID)]
    pub smart_wallet_authenticator: Box<Account<'info, crate::state::SmartWalletAuthenticator>>,
    pub whitelist_rule_programs: Box<Account<'info, crate::state::WhitelistRulePrograms>>,
    /// CHECK
    pub authenticator_program: UncheckedAccount<'info>,
    /// CHECK
    pub cpi_program: UncheckedAccount<'info>,
    pub config: Box<Account<'info, crate::state::Config>>,
    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,
}
