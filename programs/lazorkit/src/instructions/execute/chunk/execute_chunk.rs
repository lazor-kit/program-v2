use anchor_lang::prelude::*;

use crate::security::validation;
use crate::state::{Chunk, WalletState};
use crate::utils::{execute_cpi, PdaSigner};
use crate::{constants::SMART_WALLET_SEED, ID};
use anchor_lang::solana_program::hash::{HASH_BYTES, Hasher, hash};

/// Execute a previously created chunk
/// Always returns Ok(()) - errors are logged but don't fail the transaction
pub fn execute_chunk(
    ctx: Context<ExecuteChunk>,
    instruction_data_list: Vec<Vec<u8>>,
    split_index: Vec<u8>,
) -> Result<()> {
    // Cache frequently accessed values
    let cpi_accounts = &ctx.remaining_accounts;
    let smart_wallet_key = ctx.accounts.smart_wallet.key();
    let wallet_id = ctx.accounts.wallet_state.wallet_id;
    let wallet_bump = ctx.accounts.wallet_state.bump;
    let chunk = &ctx.accounts.chunk;
    let authorized_timestamp = chunk.authorized_timestamp;
    let expected_cpi_hash = chunk.cpi_hash;

    // Validate owner first (fail fast)
    if chunk.owner_wallet_address != smart_wallet_key {
        msg!("InvalidAccountOwner: Invalid account owner: expected={}, got={}", smart_wallet_key, chunk.owner_wallet_address);
        return Ok(());
    }

    // Get current timestamp
    let now = match Clock::get() {
        Ok(clock) => clock.unix_timestamp,
        Err(e) => {
            msg!("InvalidInstruction: Error getting clock: {:?}", e);
            return Ok(());
        }
    };
    
    // Validate timestamp - check if chunk is expired
    let session_end = authorized_timestamp + crate::security::MAX_SESSION_TTL_SECONDS * 2;
    if now > session_end {
        msg!("TransactionTooOld: Chunk expired: now={}, session_end={}", now, session_end);
        // Chunk will be closed automatically due to close = session_refund constraint
        return Ok(());
    }

    // Validate instruction data
    if instruction_data_list.is_empty() {
        msg!("InvalidInstructionData: Instruction data list is empty");
        return Ok(());
    }

    if instruction_data_list.len() != split_index.len() + 1 {
        msg!("InvalidInstructionData: Invalid instruction data: instruction_data_list.len()={}, split_index.len()={}", 
             instruction_data_list.len(), split_index.len());
        return Ok(());
    }

    // Serialize CPI data
    let instruction_count: u32 = match instruction_data_list.len().try_into() {
        Ok(count) => count,
        Err(e) => {
            msg!("InvalidInstructionData: Failed to convert instruction count: {:?}", e);
            return Ok(());
        }
    };

    let mut serialized_cpi_data = Vec::new();
    serialized_cpi_data.extend_from_slice(&instruction_count.to_le_bytes());
    
    for instruction_data in &instruction_data_list {
        let data_len: u32 = match instruction_data.len().try_into() {
            Ok(len) => len,
            Err(e) => {
                msg!("InvalidInstructionData: Failed to convert instruction data length: {:?}", e);
                return Ok(());
            }
        };
        serialized_cpi_data.extend_from_slice(&data_len.to_le_bytes());
        serialized_cpi_data.extend_from_slice(instruction_data);
    }

    let cpi_data_hash = hash(&serialized_cpi_data).to_bytes();

    // Hash CPI accounts
    let mut rh = Hasher::default();
    for account in cpi_accounts.iter() {
        rh.hash(account.key().as_ref());
        rh.hash(&[account.is_signer as u8]);
        rh.hash(&[account.is_writable as u8]);
    }
    let cpi_accounts_hash = rh.result().to_bytes();

    let mut cpi_combined = [0u8; HASH_BYTES * 2];
    cpi_combined[..HASH_BYTES].copy_from_slice(&cpi_data_hash);
    cpi_combined[HASH_BYTES..].copy_from_slice(&cpi_accounts_hash);
    let cpi_hash = hash(&cpi_combined).to_bytes();

    // Validate hash
    if cpi_hash != expected_cpi_hash {
        msg!("HashMismatch: Hash mismatch: expected={:?}, got={:?}", expected_cpi_hash, cpi_hash);
        return Ok(());
    }

    // Calculate account ranges
    let account_ranges = match crate::utils::calculate_account_ranges(cpi_accounts, &split_index) {
        Ok(ranges) => ranges,
        Err(e) => {
            msg!("InvalidInstructionData: Failed to calculate account ranges: {:?}", e);
            return Ok(());
        }
    };

    // Validate programs in ranges
    if let Err(e) = crate::utils::validate_programs_in_ranges(cpi_accounts, &account_ranges) {
        msg!("InvalidInstruction: Failed to validate programs in ranges: {:?}", e);
        return Ok(());
    }

    // Execute CPI instructions
    let wallet_signer = PdaSigner {
        seeds: vec![SMART_WALLET_SEED.to_vec(), wallet_id.to_le_bytes().to_vec()],
        bump: wallet_bump,
    };

    for (idx, (cpi_data, &(range_start, range_end))) in
        instruction_data_list.iter().zip(account_ranges.iter()).enumerate()
    {
        let instruction_accounts = &cpi_accounts[range_start..range_end];
        let program_account = &instruction_accounts[0];
        let instruction_accounts = &instruction_accounts[1..];

        if let Err(e) = execute_cpi(instruction_accounts, cpi_data, program_account, &wallet_signer) {
            msg!("InvalidInstruction: Failed to execute CPI instruction {}: {:?}", idx, e);
            return Ok(());
        }
    }
    
    let last_nonce = ctx.accounts.wallet_state.last_nonce;
    ctx.accounts.wallet_state.last_nonce = validation::safe_increment_nonce(last_nonce);

    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteChunk<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, wallet_state.wallet_id.to_le_bytes().as_ref()],
        bump = wallet_state.bump,
    )]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        mut,
        seeds = [
            Chunk::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            &chunk.authorized_nonce.to_le_bytes(),
        ], 
        close = session_refund,
        owner = ID,
        bump,
    )]
    pub chunk: Account<'info, Chunk>,

    #[account(mut, address = chunk.rent_refund_address)]
    /// CHECK: Validated to match chunk.rent_refund_address
    pub session_refund: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
