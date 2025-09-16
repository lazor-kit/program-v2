use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::Hasher;

use crate::security::validation;
use crate::state::{EphemeralAuthorization, LazorKitVault, ProgramConfig, SmartWalletData};
use crate::utils::{execute_cpi, PdaSigner};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

pub fn execute_ephemeral_authorization(
    ctx: Context<ExecuteEphemeralAuthorization>,
    instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
    split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
    _vault_index: u8,                     // Random vault index (0-31) calculated off-chain
) -> Result<()> {
    let cpi_accounts = &ctx.remaining_accounts[..];

    // Validate remaining accounts
    if validation::validate_remaining_accounts(&cpi_accounts).is_err() {
        return Ok(());
    }

    let authorization = &mut ctx.accounts.ephemeral_authorization;

    // Check expiry
    let now = Clock::get()?.unix_timestamp;
    if authorization.expires_at < now {
        return Ok(());
    }

    // Validate authorization owner matches smart wallet
    if authorization.owner_wallet_address != ctx.accounts.smart_wallet.key() {
        return Ok(());
    }

    // Validate ephemeral key is the signer
    require!(
        ctx.accounts.ephemeral_signer.key() == authorization.ephemeral_public_key,
        LazorKitError::InvalidAuthority
    );

    // Validate input: for n instructions, we need n-1 split indices
    require!(
        !instruction_data_list.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );
    require!(
        instruction_data_list.len() == split_index.len() + 1,
        LazorKitError::InvalidInstructionData
    );

    // Verify entire instruction_data_list hash matches session
    let serialized_cpi_data = instruction_data_list
        .try_to_vec()
        .map_err(|_| LazorKitError::InvalidInstructionData)?;
    let data_hash = anchor_lang::solana_program::hash::hash(&serialized_cpi_data).to_bytes();
    if data_hash != authorization.instruction_data_hash {
        return Ok(());
    }

    // Verify entire accounts vector hash matches session
    let mut all_accounts_hasher = Hasher::default();
    for acc in cpi_accounts.iter() {
        all_accounts_hasher.hash(acc.key.as_ref());
        all_accounts_hasher.hash(&[acc.is_signer as u8]);
        all_accounts_hasher.hash(&[acc.is_writable as u8]);
    }
    if all_accounts_hasher.result().to_bytes() != authorization.accounts_metadata_hash {
        return Ok(());
    }

    // Split accounts based on split_index and validate programs
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

    // Validate each instruction's programs for security
    for (_i, &(range_start, range_end)) in account_ranges.iter().enumerate() {
        let instruction_accounts = &cpi_accounts[range_start..range_end];

        require!(
            !instruction_accounts.is_empty(),
            LazorKitError::InsufficientCpiAccounts
        );

        // First account in each instruction slice is the program ID
        let program_account = &instruction_accounts[0];

        // Validate program is executable
        if !program_account.executable {
            return Ok(());
        }

        // Ensure program is not this program (prevent reentrancy)
        if program_account.key() == crate::ID {
            return Ok(());
        }
    }

    // Create wallet signer
    let wallet_signer = PdaSigner {
        seeds: vec![
            SMART_WALLET_SEED.to_vec(),
            ctx.accounts
                .smart_wallet_data
                .wallet_id
                .to_le_bytes()
                .to_vec(),
        ],
        bump: ctx.accounts.smart_wallet_data.bump,
    };

    // Execute all instructions using the same account ranges
    for (_i, (cpi_data, &(range_start, range_end))) in instruction_data_list
        .iter()
        .zip(account_ranges.iter())
        .enumerate()
    {
        let instruction_accounts = &cpi_accounts[range_start..range_end];

        // First account is the program, rest are instruction accounts
        let program_account = &instruction_accounts[0];
        let instruction_accounts = &instruction_accounts[1..];

        let exec_res = execute_cpi(
            instruction_accounts,
            cpi_data,
            program_account,
            wallet_signer.clone(),
        );

        if exec_res.is_err() {
            return Ok(());
        }
    }

    // Validate that the provided vault matches the vault index from the session
    let vault_validation = crate::state::LazorKitVault::validate_vault_for_index(
        &ctx.accounts.lazorkit_vault.key(),
        authorization.vault_index,
        &crate::ID,
    );

    // Distribute fees gracefully (don't fail if fees can't be paid or vault validation fails)
    if vault_validation.is_ok() {
        crate::utils::distribute_fees(
            &ctx.accounts.config,
            &ctx.accounts.smart_wallet.to_account_info(),
            &ctx.accounts.fee_payer.to_account_info(),
            &ctx.accounts.referral.to_account_info(),
            &ctx.accounts.lazorkit_vault.to_account_info(),
            &ctx.accounts.system_program,
            wallet_signer,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
#[instruction(instruction_data_list: Vec<Vec<u8>>, split_index: Vec<u8>, vault_index: u8)]
pub struct ExecuteEphemeralAuthorization<'info> {
    /// Fee payer for the transaction (stored in authorization)
    #[account(mut, address = ephemeral_authorization.fee_payer_address)]
    pub fee_payer: Signer<'info>,

    /// Ephemeral key that can sign transactions (must be signer)
    #[account(address = ephemeral_authorization.ephemeral_public_key)]
    pub ephemeral_signer: Signer<'info>,

    #[account(seeds = [ProgramConfig::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, ProgramConfig>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_data.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_data.bump,
        
    )]
    /// CHECK: PDA verified
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletData::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_data: Box<Account<'info, SmartWalletData>>,

    /// CHECK: referral account (matches smart_wallet_data.referral)
    #[account(mut, address = smart_wallet_data.referral_address)]
    pub referral: UncheckedAccount<'info>,

    /// LazorKit vault (empty PDA that holds SOL) - random vault selected by client
    #[account(
        mut,
        seeds = [LazorKitVault::PREFIX_SEED, &vault_index.to_le_bytes()],
        bump,
    )]
    /// CHECK: Empty PDA vault that only holds SOL, validated to be correct random vault
    pub lazorkit_vault: SystemAccount<'info>,

    /// Ephemeral authorization to execute. Closed on success to refund rent.
    #[account(mut, close = authorization_refund, owner = ID)]
    pub ephemeral_authorization: Account<'info, EphemeralAuthorization>,

    /// CHECK: rent refund destination (stored in authorization)
    #[account(mut, address = ephemeral_authorization.rent_refund_address)]
    pub authorization_refund: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
