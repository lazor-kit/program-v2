use anchor_lang::prelude::*;

use crate::error::LazorKitError;
use crate::security::validation;
use crate::state::{LazorKitVault, Config, SmartWalletConfig, Chunk};
use crate::utils::{execute_cpi, PdaSigner};
use crate::{constants::SMART_WALLET_SEED, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

/// Execute a chunk from the chunk buffer
///
/// Executes a chunk from the previously created buffer. Used when the main
/// execute transaction is too large and needs to be split into smaller,
/// manageable pieces for processing.
pub fn execute_chunk(
    ctx: Context<ExecuteChunk>,
    instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
    split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
) -> Result<()> {
    // Step 1: Prepare and validate input parameters
    let cpi_accounts = &ctx.remaining_accounts[..];

    // Validate remaining accounts format (graceful abort on failure)
    if validation::validate_remaining_accounts(&cpi_accounts).is_err() {
        msg!("Failed validation: remaining accounts validation failed");
        return Ok(());
    }

    let session = &mut ctx.accounts.chunk;

    // Step 2: Validate session state and authorization
    // Check if the chunk session has expired
    let now = Clock::get()?.unix_timestamp;
    if session.expires_at < now {
        msg!("Failed validation: session expired. expires_at: {}, now: {}", session.expires_at, now);
        return Ok(());
    }

    // Verify the chunk belongs to the correct smart wallet
    if session.owner_wallet_address != ctx.accounts.smart_wallet.key() {
        msg!("Failed validation: wallet address mismatch. session: {}, smart_wallet: {}", session.owner_wallet_address, ctx.accounts.smart_wallet.key());
        return Ok(());
    }

    // Step 3: Validate instruction data and split indices
    // For n instructions, we need n-1 split indices to divide the accounts
    require!(
        !instruction_data_list.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );
    require!(
        instruction_data_list.len() == split_index.len() + 1,
        LazorKitError::InvalidInstructionData
    );

    // Step 4: Verify instruction data integrity
    // Serialize and hash the instruction data to match the session
    let serialized_cpi_data = instruction_data_list
        .try_to_vec()
        .map_err(|_| LazorKitError::InvalidInstructionData)?;

    let data_hash = hash(&serialized_cpi_data).to_bytes();
    if data_hash != session.instruction_data_hash {
        msg!("Failed validation: instruction data hash mismatch. computed: {:?}, session: {:?}", data_hash, session.instruction_data_hash);
        return Ok(());
    }

    // Step 5: Split accounts based on split indices
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

    // Step 6: Verify accounts metadata hash matches session
    let mut all_accounts_hasher = Hasher::default();
    for acc in cpi_accounts.iter() {
        all_accounts_hasher.hash(acc.key.as_ref());
        all_accounts_hasher.hash(&[acc.is_signer as u8]);
        all_accounts_hasher.hash(&[acc.is_writable as u8]);
    }
    if all_accounts_hasher.result().to_bytes() != session.accounts_metadata_hash {
        msg!("Failed validation: accounts metadata hash mismatch");
        return Ok(());
    }

    // Step 7: Validate each instruction's programs for security
    for (_i, &(range_start, range_end)) in account_ranges.iter().enumerate() {
        let instruction_accounts = &cpi_accounts[range_start..range_end];

        require!(
            !instruction_accounts.is_empty(),
            LazorKitError::InsufficientCpiAccounts
        );

        // First account in each instruction slice is the program ID
        let program_account = &instruction_accounts[0];

        // Validate program is executable (not a data account)
        if !program_account.executable {
            msg!("Failed validation: program not executable. program: {}", program_account.key());
            return Ok(());
        }

        // Prevent reentrancy attacks by blocking calls to this program
        if program_account.key() == crate::ID {
            msg!("Failed validation: reentrancy attempt detected. program: {}", program_account.key());
            return Ok(());
        }
    }

    // Step 8: Create wallet signer for CPI execution
    let wallet_signer = PdaSigner {
        seeds: vec![
            SMART_WALLET_SEED.to_vec(),
            ctx.accounts
                .smart_wallet_config
                .wallet_id
                .to_le_bytes()
                .to_vec(),
        ],
        bump: ctx.accounts.smart_wallet_config.bump,
    };

    // Step 9: Execute all instructions using the account ranges
    for (_i, (cpi_data, &(range_start, range_end))) in
        instruction_data_list.iter().zip(account_ranges.iter()).enumerate()
    {
        let instruction_accounts = &cpi_accounts[range_start..range_end];

        // First account is the program, rest are instruction accounts
        let program_account = &instruction_accounts[0];
        let instruction_accounts = &instruction_accounts[1..];

        // Execute the CPI instruction (graceful abort on failure)
        let exec_res = execute_cpi(
            instruction_accounts,
            cpi_data,
            program_account,
            wallet_signer.clone(),
        );

        if exec_res.is_err() {
            msg!("Failed execution: CPI instruction failed. error: {:?}", exec_res.err());
            return Ok(());
        }
    }

    crate::utils::distribute_fees(
        &ctx.accounts.config,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        wallet_signer,
    )?;


    msg!("Successfully executed deferred transaction");
    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteChunk<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [Config::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
    )]
    /// CHECK: PDA verified
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    /// CHECK: referral account (matches smart_wallet_config.referral)
    #[account(mut, address = smart_wallet_config.referral_address)]
    pub referral: UncheckedAccount<'info>,

    /// LazorKit vault (empty PDA that holds SOL) - random vault selected by client
    #[account(
        mut,
        seeds = [LazorKitVault::PREFIX_SEED, &chunk.vault_index.to_le_bytes()],
        bump,
    )]
    /// CHECK: Empty PDA vault that only holds SOL, validated to be correct random vault
    pub lazorkit_vault: SystemAccount<'info>,

    /// Transaction session to execute. Closed on success to refund rent.
    #[account(
        mut,
        seeds = [
            Chunk::PREFIX_SEED,
            smart_wallet.key.as_ref(),
            &chunk.authorized_nonce.to_le_bytes(),
        ], 
        close = session_refund, 
        owner = ID,
        bump,
    )]
    pub chunk: Account<'info, Chunk>,

    /// CHECK: rent refund destination (stored in session)
    #[account(mut, address = chunk.rent_refund_address)]
    pub session_refund: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
