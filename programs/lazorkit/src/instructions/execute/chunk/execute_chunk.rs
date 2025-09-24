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

    // Validate remaining accounts format
    validation::validate_remaining_accounts(&cpi_accounts)?;

    let chunk = &mut ctx.accounts.chunk;

    // Step 2: Validate session state and authorization
    // Check if the chunk session has expired based on timestamp
    let now = Clock::get()?.unix_timestamp;
    require!(
        chunk.authorized_timestamp >= now - crate::security::MAX_SESSION_TTL_SECONDS,
        LazorKitError::TransactionTooOld
    );

    // Verify the chunk belongs to the correct smart wallet
    require!(
        chunk.owner_wallet_address == ctx.accounts.smart_wallet.key(),
        LazorKitError::InvalidAccountOwner
    );

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
    // Serialize CPI data to match client-side format (length + data for each instruction)
    let mut serialized_cpi_data = Vec::new();
    serialized_cpi_data.extend_from_slice(&(instruction_data_list.len() as u32).to_le_bytes());
    
    for instruction_data in &instruction_data_list {
        serialized_cpi_data.extend_from_slice(&(instruction_data.len() as u32).to_le_bytes());
        serialized_cpi_data.extend_from_slice(instruction_data);
    }
    
    let cpi_data_hash = hash(&serialized_cpi_data).to_bytes();
    
    // Hash CPI accounts to match client-side format
    // Client-side includes program_id for each instruction, so we need to account for that
    let mut rh = Hasher::default();
    for account in cpi_accounts.iter() {
        rh.hash(account.key().as_ref());
        rh.hash(&[account.is_signer as u8]);
        rh.hash(&[account.is_writable as u8]);
    }
    let cpi_accounts_hash = rh.result().to_bytes();
    
    // Combine CPI hashes
    let mut cpi_combined = Vec::new();
    cpi_combined.extend_from_slice(&cpi_data_hash);
    cpi_combined.extend_from_slice(&cpi_accounts_hash);
    let cpi_hash = hash(&cpi_combined).to_bytes();
    
    // Verify the combined CPI hash matches the chunk
    require!(
        cpi_hash == chunk.cpi_hash,
        LazorKitError::HashMismatch
    );

    // Step 5: Split accounts based on split indices
    let account_ranges = crate::utils::calculate_account_ranges(cpi_accounts, &split_index)?;

    // Step 6: Accounts metadata validation is now covered by CPI hash validation above

    // Step 7: Validate each instruction's programs for security
    crate::utils::validate_programs_in_ranges(cpi_accounts, &account_ranges)?;

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

        // Execute the CPI instruction
        execute_cpi(
            instruction_accounts,
            cpi_data,
            program_account,
            wallet_signer.clone(),
        )?;
    }

    crate::utils::handle_fee_distribution(
        &ctx.accounts.config,
        &ctx.accounts.smart_wallet_config,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        chunk.vault_index,
    )?;


    msg!("Successfully executed chunk transaction: wallet={}, nonce={}, instructions={}", 
         ctx.accounts.smart_wallet.key(), 
         chunk.authorized_nonce,
         instruction_data_list.len());
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

    /// Transaction session to execute. Closed to refund rent.
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
