use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::Hasher;

use crate::error::LazorKitError;
use crate::security::validation;
use crate::state::{Config, SmartWallet, TransactionSession};
use crate::utils::{execute_cpi, PdaSigner};
use crate::{constants::SMART_WALLET_SEED, ID};

pub fn execute_session_transaction(
    ctx: Context<ExecuteSessionTransaction>,
    vec_cpi_data: Vec<Vec<u8>>,
    split_index: Vec<u8>,
) -> Result<()> {
    let cpi_accounts = &ctx.remaining_accounts[..];

    // We'll gracefully abort (close the commit and return Ok) if any binding check fails.
    // Only hard fail on obviously invalid input sizes.
    if validation::validate_remaining_accounts(&cpi_accounts).is_err() {
        msg!("Invalid remaining accounts; closing session with refund due to graceful flag");
        return Ok(());
    }

    let session = &mut ctx.accounts.transaction_session;

    // Expiry and usage
    let now = Clock::get()?.unix_timestamp;
    if session.expires_at < now {
        msg!("Transaction session expired");
        return Ok(());
    }

    // Bind wallet and target program
    if session.owner_wallet != ctx.accounts.smart_wallet.key() {
        msg!("The session owner does not match with smart wallet");
        return Ok(());
    }

    // Validate the transaction_session PDA derived from (wallet, authorized_nonce)
    let expected_session = Pubkey::find_program_address(
        &[
            TransactionSession::PREFIX_SEED,
            ctx.accounts.smart_wallet.key.as_ref(),
            &session.authorized_nonce.to_le_bytes(),
        ],
        &crate::ID,
    )
    .0;
    if expected_session != session.key() {
        msg!("Invalid transaction session PDA");
        return Ok(());
    }

    // Validate input: for n instructions, we need n-1 split indices
    require!(
        !vec_cpi_data.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );
    require!(
        vec_cpi_data.len() == split_index.len() + 1,
        LazorKitError::InvalidInstructionData
    );

    // Verify entire vec_cpi_data hash matches session
    let serialized_cpi_data = vec_cpi_data
        .try_to_vec()
        .map_err(|_| LazorKitError::InvalidInstructionData)?;
    let data_hash = anchor_lang::solana_program::hash::hash(&serialized_cpi_data).to_bytes();
    if data_hash != session.data_hash {
        msg!("Transaction data vector does not match session");
        return Ok(());
    }

    // Split accounts based on split_index and verify each instruction
    let mut account_ranges = Vec::new();
    let mut start = 0usize;

    // Calculate account ranges for each instruction using split indices
    for &split_point in split_index.iter() {
        let end = split_point as usize;
        require!(
            end > start && end <= cpi_accounts.len(),
            LazorKitError::AccountSliceOutOfBounds
        );
        account_ranges.push((start, end));
        start = end;
    }

    // Add the last instruction range (from last split to end)
    require!(
        start < cpi_accounts.len(),
        LazorKitError::AccountSliceOutOfBounds
    );
    account_ranges.push((start, cpi_accounts.len()));

    // Verify entire accounts vector hash matches session
    let mut all_accounts_hasher = Hasher::default();
    for acc in cpi_accounts.iter() {
        all_accounts_hasher.hash(acc.key.as_ref());
        all_accounts_hasher.hash(&[acc.is_signer as u8]);
        all_accounts_hasher.hash(&[acc.is_writable as u8]);
    }
    if all_accounts_hasher.result().to_bytes() != session.accounts_hash {
        msg!("Transaction accounts vector does not match session");
        return Ok(());
    }

    // Validate each instruction's programs for security
    for (i, &(range_start, range_end)) in account_ranges.iter().enumerate() {
        let instruction_accounts = &cpi_accounts[range_start..range_end];

        require!(
            !instruction_accounts.is_empty(),
            LazorKitError::InsufficientCpiAccounts
        );

        // First account in each instruction slice is the program ID
        let program_account = &instruction_accounts[0];

        // Validate program is executable
        if !program_account.executable {
            msg!("Program at index {} must be executable", i);
            return Ok(());
        }

        // Ensure program is not this program (prevent reentrancy)
        if program_account.key() == crate::ID {
            msg!("Reentrancy detected at instruction {}", i);
            return Ok(());
        }
    }

    // Create wallet signer
    let wallet_signer = PdaSigner {
        seeds: vec![
            SMART_WALLET_SEED.to_vec(),
            ctx.accounts.smart_wallet_data.id.to_le_bytes().to_vec(),
        ],
        bump: ctx.accounts.smart_wallet_data.bump,
        owner: anchor_lang::system_program::ID,
    };

    // Execute all instructions using the same account ranges
    for (i, (cpi_data, &(range_start, range_end))) in
        vec_cpi_data.iter().zip(account_ranges.iter()).enumerate()
    {
        let instruction_accounts = &cpi_accounts[range_start..range_end];

        // First account is the program, rest are instruction accounts
        let program_account = &instruction_accounts[0];
        let instruction_accounts = &instruction_accounts[1..];

        msg!(
            "Executing instruction {} to program: {}",
            i,
            program_account.key()
        );

        let exec_res = execute_cpi(
            instruction_accounts,
            cpi_data,
            program_account,
            wallet_signer.clone(),
            &[ctx.accounts.payer.key()],
        );

        if exec_res.is_err() {
            msg!(
                "CPI {} failed; closing session with refund due to graceful flag",
                i
            );
            return Ok(());
        }
    }

    msg!(
        "All {} instructions executed successfully",
        vec_cpi_data.len()
    );

    // Validate that the provided vault matches the vault index from the session
    let vault_validation = crate::state::LazorKitVault::validate_vault_for_index(
        &ctx.accounts.lazorkit_vault.key(),
        session.vault_index,
        &crate::ID,
    );

    // Distribute fees gracefully (don't fail if fees can't be paid or vault validation fails)
    if vault_validation.is_ok() {
        crate::utils::distribute_fees_graceful(
            &ctx.accounts.config,
            &ctx.accounts.smart_wallet.to_account_info(),
            &ctx.accounts.payer.to_account_info(),
            &ctx.accounts.referral.to_account_info(),
            &ctx.accounts.lazorkit_vault.to_account_info(),
            &ctx.accounts.system_program,
            wallet_signer,
            session.authorized_nonce,
        );
    }

    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteSessionTransaction<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_data.id.to_le_bytes().as_ref()],
        bump = smart_wallet_data.bump,
        owner = system_program.key(),
    )]
    /// CHECK: PDA verified
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWallet::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_data: Box<Account<'info, SmartWallet>>,

    /// CHECK: referral account (matches smart_wallet_data.referral)
    #[account(mut, address = smart_wallet_data.referral)]
    pub referral: UncheckedAccount<'info>,

    /// LazorKit vault (empty PDA that holds SOL) - random vault selected by client
    #[account(mut, owner = crate::ID)]
    /// CHECK: Empty PDA vault that only holds SOL, validated to be correct random vault
    pub lazorkit_vault: UncheckedAccount<'info>,

    /// Transaction session to execute. Closed on success to refund rent.
    #[account(mut, close = session_refund, owner = ID)]
    pub transaction_session: Account<'info, TransactionSession>,

    /// CHECK: rent refund destination (stored in session)
    #[account(mut, address = transaction_session.rent_refund_to)]
    pub session_refund: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
