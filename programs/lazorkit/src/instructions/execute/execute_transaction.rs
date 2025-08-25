use anchor_lang::prelude::*;

use crate::instructions::{Args as _, ExecuteTransactionArgs};
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

pub fn execute_transaction<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ExecuteTransaction<'info>>,
    args: ExecuteTransactionArgs,
) -> Result<()> {
    // 0. Validate args and global state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;

    // 0.1 Verify authorization and parse typed message
    let msg: ExecuteMessage = verify_authorization(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.wallet_device,
        ctx.accounts.smart_wallet.key(),
        args.passkey_pubkey,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        ctx.accounts.smart_wallet_data.last_nonce,
    )?;

    // 1. Validate and check policy program
    let policy_program_info = &ctx.accounts.policy_program;

    // Ensure policy program is executable
    validation::validate_program_executable(policy_program_info)?;

    // Verify policy program is registered
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &policy_program_info.key(),
    )?;

    // Ensure policy program matches wallet configuration
    require!(
        policy_program_info.key() == ctx.accounts.smart_wallet_data.policy_program,
        LazorKitError::InvalidProgramAddress
    );

    // 2. Prepare PDA signer for policy CPI
    let policy_signer = get_pda_signer(
        &args.passkey_pubkey,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // 3. Split remaining accounts
    let (policy_accounts, cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, args.split_index)?;

    // Validate account counts
    require!(
        !policy_accounts.is_empty(),
        LazorKitError::InsufficientPolicyAccounts
    );

    // 4. Verify policy discriminator on provided policy_data
    let policy_data = &args.policy_data;
    require!(
        policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidCheckPolicyDiscriminator
    );

    // 4.1 Validate policy_data size and compare hash from message
    validation::validate_policy_data(policy_data)?;
    require!(
        hash(policy_data).to_bytes() == msg.policy_data_hash,
        LazorKitError::InvalidInstructionData
    );

    // 4.2 Compare policy accounts hash against message
    let mut rh = Hasher::default();
    rh.hash(policy_program_info.key.as_ref());
    for acc in policy_accounts.iter() {
        rh.hash(acc.key.as_ref());
        rh.hash(&[acc.is_signer as u8]);
        rh.hash(&[acc.is_writable as u8]);
    }
    require!(
        rh.result().to_bytes() == msg.policy_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // 5. Execute policy CPI to check if the transaction is allowed
    msg!(
        "Executing policy check for smart wallet: {}",
        ctx.accounts.smart_wallet.key()
    );

    execute_cpi(
        policy_accounts,
        policy_data,
        policy_program_info,
        policy_signer,
        &[],
    )?;

    msg!("Policy check passed");

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
        ch.hash(&[acc.is_signer as u8]);
        ch.hash(&[acc.is_writable as u8]);
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
                ctx.accounts.smart_wallet_data.id.to_le_bytes().to_vec(),
            ],
            bump: ctx.accounts.smart_wallet_data.bump,
        };

        msg!(
            "Executing CPI to program: {}",
            ctx.accounts.cpi_program.key()
        );
        execute_cpi(
            cpi_accounts,
            &args.cpi_data,
            &ctx.accounts.cpi_program,
            wallet_signer,
            &[ctx.accounts.payer.key()],
        )?;
    }

    msg!("Transaction executed successfully");
    // 8. Increment nonce
    ctx.accounts.smart_wallet_data.last_nonce = ctx
        .accounts
        .smart_wallet_data
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_data.id.to_le_bytes().as_ref()],
        bump = smart_wallet_data.bump,
        owner = crate::ID,
    )]
    /// CHECK: PDA verified by seeds
    pub smart_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [crate::state::SmartWallet::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = crate::ID,
    )]
    pub smart_wallet_data: Box<Account<'info, crate::state::SmartWallet>>,

    #[account(owner = crate::ID)]
    pub wallet_device: Box<Account<'info, crate::state::WalletDevice>>,
    #[account(
        seeds = [crate::state::PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = crate::ID
    )]
    pub policy_program_registry: Box<Account<'info, crate::state::PolicyProgramRegistry>>,
    /// CHECK: must be executable (policy program)
    #[account(executable)]
    pub policy_program: UncheckedAccount<'info>,
    /// CHECK: must be executable (target program)
    #[account(executable)]
    pub cpi_program: UncheckedAccount<'info>,
    #[account(
        seeds = [crate::state::Config::PREFIX_SEED],
        bump,
        owner = crate::ID
    )]
    pub config: Box<Account<'info, crate::state::Config>>,
    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,
}
