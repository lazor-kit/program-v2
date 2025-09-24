use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::Hasher;

use crate::security::validation;
use crate::state::{Permission, LazorKitVault, Config, SmartWalletConfig};
use crate::utils::{execute_cpi, PdaSigner};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

/// Execute transactions using ephemeral permission
///
/// Executes transactions using a previously granted ephemeral key, allowing
/// multiple operations without repeated passkey authentication. Perfect for
/// games or applications that require frequent interactions with the wallet.
pub fn execute_with_permission(
    ctx: Context<ExecuteWithPermission>,
    instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
    split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
) -> Result<()> {
    // Step 1: Prepare and validate input parameters
    let cpi_accounts = &ctx.remaining_accounts[..];

    // Validate remaining accounts format
    validation::validate_remaining_accounts(&cpi_accounts)?;

    let authorization = &mut ctx.accounts.permission;

    // Step 2: Validate permission state and authorization
    // Check if the permission has expired
    let now = Clock::get()?.unix_timestamp;
    require!(
        authorization.expires_at >= now,
        LazorKitError::TransactionTooOld
    );

    // Verify the permission belongs to the correct smart wallet
    require!(
        authorization.owner_wallet_address == ctx.accounts.smart_wallet.key(),
        LazorKitError::InvalidAccountOwner
    );

    // Verify the ephemeral key matches the permission's authorized key
    require!(
        ctx.accounts.ephemeral_signer.key() == authorization.ephemeral_public_key,
        LazorKitError::InvalidAuthority
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
    // Serialize and hash the instruction data to match the permission
    let serialized_cpi_data = instruction_data_list
        .try_to_vec()
        .map_err(|_| LazorKitError::InvalidInstructionData)?;
    let data_hash = anchor_lang::solana_program::hash::hash(&serialized_cpi_data).to_bytes();
    require!(
        data_hash == authorization.instruction_data_hash,
        LazorKitError::HashMismatch
    );

    // Step 5: Verify accounts metadata integrity
    // Hash all accounts to ensure they match the permission
    let mut all_accounts_hasher = Hasher::default();
    for acc in cpi_accounts.iter() {
        all_accounts_hasher.hash(acc.key.as_ref());
        all_accounts_hasher.hash(&[acc.is_signer as u8]);
        all_accounts_hasher.hash(&[acc.is_writable as u8]);
    }
    require!(
        all_accounts_hasher.result().to_bytes() == authorization.accounts_metadata_hash,
        LazorKitError::HashMismatch
    );

    // Step 6: Split accounts based on split indices
    let account_ranges = crate::utils::calculate_account_ranges(cpi_accounts, &split_index)?;

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
    for (_i, (cpi_data, &(range_start, range_end))) in instruction_data_list
        .iter()
        .zip(account_ranges.iter())
        .enumerate()
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

    // Step 10: Handle fee distribution and vault validation
    crate::utils::handle_fee_distribution(
        &ctx.accounts.config,
        &ctx.accounts.smart_wallet_config,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.fee_payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        authorization.vault_index,
    )?;

    msg!("Successfully executed permission transaction: wallet={}, ephemeral_key={}, instructions={}", 
         ctx.accounts.smart_wallet.key(), 
         authorization.ephemeral_public_key,
         instruction_data_list.len());
    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteWithPermission<'info> {
    /// Fee payer for the transaction (stored in authorization)
    #[account(mut, address = permission.fee_payer_address)]
    pub fee_payer: Signer<'info>,

    /// Ephemeral key that can sign transactions (must be signer)
    #[account(address = permission.ephemeral_public_key)]
    pub ephemeral_signer: Signer<'info>,

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
        seeds = [LazorKitVault::PREFIX_SEED, &permission.vault_index.to_le_bytes()],
        bump,
    )]
    /// CHECK: Empty PDA vault that only holds SOL, validated to be correct random vault
    pub lazorkit_vault: SystemAccount<'info>,

    /// Ephemeral authorization to execute. Closed on success to refund rent.
    #[account(mut, close = authorization_refund, owner = ID)]
    pub permission: Account<'info, Permission>,

    /// CHECK: rent refund destination (stored in authorization)
    #[account(mut, address = permission.rent_refund_address)]
    pub authorization_refund: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
