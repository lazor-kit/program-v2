use anchor_lang::prelude::*;

use crate::constants::SOL_TRANSFER_DISCRIMINATOR;
use crate::error::LazorKitError;
use crate::security::validation;
use crate::state::{Config, CpiCommit, SmartWalletConfig};
use crate::utils::{execute_cpi, transfer_sol_from_pda, PdaSigner};
use crate::{constants::SMART_WALLET_SEED, ID};

pub fn execute_committed(ctx: Context<ExecuteCommitted>, cpi_data: Vec<u8>) -> Result<()> {
    // We'll gracefully abort (close the commit and return Ok) if any binding check fails.
    // Only hard fail on obviously invalid input sizes.
    if let Err(_) = validation::validate_remaining_accounts(&ctx.remaining_accounts) {
        return Ok(()); // graceful no-op; account will still be closed below
    }

    let commit = &mut ctx.accounts.cpi_commit;

    // Expiry and usage
    let now = Clock::get()?.unix_timestamp;
    if commit.expires_at < now {
        return Ok(());
    }

    // Bind wallet and target program
    if commit.owner_wallet != ctx.accounts.smart_wallet.key() {
        return Ok(());
    }

    // Validate program is executable only (no whitelist/rule checks here)
    if !ctx.accounts.cpi_program.executable {
        return Ok(());
    }

    // Compute accounts hash from remaining accounts and compare
    let mut hasher = anchor_lang::solana_program::hash::Hasher::default();
    hasher.hash(ctx.accounts.cpi_program.key.as_ref());
    for acc in ctx.remaining_accounts.iter() {
        hasher.hash(acc.key.as_ref());
        hasher.hash(&[acc.is_writable as u8, acc.is_signer as u8]);
    }
    let computed = hasher.result().to_bytes();
    if computed != commit.accounts_hash {
        return Ok(());
    }

    // Verify data_hash bound with authorized nonce to prevent cross-commit reuse
    let data_hash = anchor_lang::solana_program::hash::hash(&cpi_data).to_bytes();
    if data_hash != commit.data_hash {
        return Ok(());
    }

    if cpi_data.get(0..4) == Some(&SOL_TRANSFER_DISCRIMINATOR)
        && ctx.accounts.cpi_program.key() == anchor_lang::solana_program::system_program::ID
    {
        // === Native SOL Transfer ===
        require!(
            ctx.remaining_accounts.len() >= 2,
            LazorKitError::SolTransferInsufficientAccounts
        );

        // Extract and validate amount
        let amount_bytes = cpi_data.get(4..12).ok_or(LazorKitError::InvalidCpiData)?;
        let amount = u64::from_le_bytes(
            amount_bytes
                .try_into()
                .map_err(|_| LazorKitError::InvalidCpiData)?,
        );

        // Validate amount
        validation::validate_lamport_amount(amount)?;

        // Ensure destination is valid
        let destination_account = &ctx.remaining_accounts[1];
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
        // Validate CPI program
        validation::validate_program_executable(&ctx.accounts.cpi_program)?;

        // Ensure CPI program is not this program (prevent reentrancy)
        require!(
            ctx.accounts.cpi_program.key() != crate::ID,
            LazorKitError::ReentrancyDetected
        );

        // Ensure sufficient accounts for CPI
        require!(
            !ctx.remaining_accounts.is_empty(),
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
            ctx.remaining_accounts,
            &cpi_data,
            &ctx.accounts.cpi_program,
            Some(wallet_signer),
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteCommitted<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
        owner = ID,
    )]
    /// CHECK: PDA verified
    pub smart_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    /// CHECK: target CPI program
    pub cpi_program: UncheckedAccount<'info>,

    /// Commit to execute. Closed on success to refund rent.
    #[account(mut, close = commit_refund)]
    pub cpi_commit: Account<'info, CpiCommit>,

    /// CHECK: rent refund destination (stored in commit)
    #[account(mut, address = cpi_commit.rent_refund_to)]
    pub commit_refund: UncheckedAccount<'info>,
}
