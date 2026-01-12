//! Token Limit Plugin for Lazorkit V2
//!
//! This plugin enforces token transfer limits per authority. It tracks remaining token amounts that can be transferred
//! and decreases the limit as operations are performed.

use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{Discriminator, Transmutable, TransmutableMut};
use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

/// Plugin instruction discriminator
#[repr(u8)]
pub enum PluginInstruction {
    CheckPermission = 0,
    UpdateState = 1,
    Initialize = 2,
}

/// Plugin config account structure
#[repr(C, align(8))]
#[derive(Debug)]
pub struct TokenLimitConfig {
    pub discriminator: u8,
    pub bump: u8,
    pub wallet_account: Pubkey, // WalletAccount this config belongs to
    pub mint: Pubkey,           // Token mint address
    pub remaining_amount: u64,  // Remaining token limit (in token decimals)
    pub _padding: [u8; 7],
}

impl TokenLimitConfig {
    pub const LEN: usize = core::mem::size_of::<Self>();
    pub const SEED: &'static [u8] = b"token_limit_config";

    pub fn new(wallet_account: Pubkey, bump: u8, mint: Pubkey, remaining_amount: u64) -> Self {
        Self {
            discriminator: Discriminator::WalletAccount as u8,
            bump,
            wallet_account,
            mint,
            remaining_amount,
            _padding: [0; 7],
        }
    }
}

impl Transmutable for TokenLimitConfig {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for TokenLimitConfig {}

/// Entry point
#[cfg(not(feature = "no-entrypoint"))]
pinocchio::entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let instruction = instruction_data[0];

    match instruction {
        0 => handle_check_permission(accounts, instruction_data),
        1 => handle_update_state(accounts, instruction_data),
        2 => handle_initialize(program_id, accounts, instruction_data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// CheckPermission instruction handler
fn handle_check_permission(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    if accounts.len() < 3 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let config_account = &accounts[0];
    let wallet_account = &accounts[1];
    let _wallet_vault = &accounts[2];

    // Validate config account
    check_self_owned(config_account, ProgramError::InvalidAccountData)?;

    // Load plugin config
    let config_data = unsafe { config_account.borrow_data_unchecked() };
    if config_data.len() < TokenLimitConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let config = unsafe { TokenLimitConfig::load_unchecked(config_data)? };

    // Validate wallet_account matches
    if config.wallet_account != *wallet_account.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Parse instruction data (Pure External format)
    // Format: [instruction: u8, authority_id: u32, authority_data_len: u32, authority_data, instruction_data_len: u32, instruction_data]
    if instruction_data.len() < 9 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let _authority_id = u32::from_le_bytes([
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
        instruction_data[4],
    ]);

    let authority_data_len = u32::from_le_bytes([
        instruction_data[5],
        instruction_data[6],
        instruction_data[7],
        instruction_data[8],
    ]) as usize;

    if instruction_data.len() < 9 + authority_data_len + 4 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let instruction_payload_len = u32::from_le_bytes([
        instruction_data[9 + authority_data_len],
        instruction_data[9 + authority_data_len + 1],
        instruction_data[9 + authority_data_len + 2],
        instruction_data[9 + authority_data_len + 3],
    ]) as usize;

    if instruction_data.len() < 9 + authority_data_len + 4 + instruction_payload_len {
        return Err(ProgramError::InvalidInstructionData);
    }

    let instruction_payload = &instruction_data
        [9 + authority_data_len + 4..9 + authority_data_len + 4 + instruction_payload_len];

    // Check if this is a token transfer instruction
    // SPL Token Transfer instruction discriminator is 3
    if instruction_payload.is_empty() || instruction_payload[0] != 3 {
        // Not a token transfer, allow it (this plugin only checks token transfers)
        return Ok(());
    }

    // Parse token transfer instruction
    // SPL Token Transfer format: [discriminator: u8, amount: u64]
    if instruction_payload.len() < 9 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let transfer_amount = u64::from_le_bytes([
        instruction_payload[1],
        instruction_payload[2],
        instruction_payload[3],
        instruction_payload[4],
        instruction_payload[5],
        instruction_payload[6],
        instruction_payload[7],
        instruction_payload[8],
    ]);

    // Check if transfer amount exceeds remaining limit
    if transfer_amount > config.remaining_amount {
        return Err(ProgramError::Custom(1)); // Permission denied - exceeds limit
    }

    // Check if instruction involves the correct mint
    // For simplicity, we'll check accounts[0] (source) and accounts[1] (destination)
    // In a real implementation, we'd need to verify the mint matches
    // For now, we'll allow if amount is within limit

    Ok(())
}

/// UpdateState instruction handler
/// Called after instruction execution to update remaining limit
fn handle_update_state(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    if accounts.len() < 1 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let config_account = &accounts[0];

    // Validate config account
    check_self_owned(config_account, ProgramError::InvalidAccountData)?;

    // Load plugin config
    let mut config_data = unsafe { config_account.borrow_mut_data_unchecked() };
    if config_data.len() < TokenLimitConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let config = unsafe { TokenLimitConfig::load_mut_unchecked(&mut config_data)? };

    // Parse instruction data
    // Format: [instruction: u8, instruction_data_len: u32, instruction_data]
    if instruction_data.len() < 5 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let instruction_payload_len = u32::from_le_bytes([
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
        instruction_data[4],
    ]) as usize;

    if instruction_data.len() < 5 + instruction_payload_len {
        return Err(ProgramError::InvalidInstructionData);
    }

    let instruction_payload = &instruction_data[5..5 + instruction_payload_len];

    // Check if this is a token transfer
    if !instruction_payload.is_empty() && instruction_payload[0] == 3 {
        // Parse transfer amount
        if instruction_payload.len() >= 9 {
            let transfer_amount = u64::from_le_bytes([
                instruction_payload[1],
                instruction_payload[2],
                instruction_payload[3],
                instruction_payload[4],
                instruction_payload[5],
                instruction_payload[6],
                instruction_payload[7],
                instruction_payload[8],
            ]);

            // Decrease remaining amount
            config.remaining_amount = config.remaining_amount.saturating_sub(transfer_amount);
        }
    }

    Ok(())
}

/// Initialize instruction handler
fn handle_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let config_account = &accounts[0];
    let wallet_account = &accounts[1];
    let payer = &accounts[2];
    let _system_program = &accounts[3];

    // Parse mint and initial amount from instruction data
    // Format: [instruction: u8, mint: 32 bytes, initial_amount: u64]
    if instruction_data.len() < 42 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut mint_bytes = [0u8; 32];
    mint_bytes.copy_from_slice(&instruction_data[1..33]);
    let mint =
        Pubkey::try_from(mint_bytes.as_ref()).map_err(|_| ProgramError::InvalidAccountData)?;

    let initial_amount = u64::from_le_bytes([
        instruction_data[33],
        instruction_data[34],
        instruction_data[35],
        instruction_data[36],
        instruction_data[37],
        instruction_data[38],
        instruction_data[39],
        instruction_data[40],
    ]);

    // Initialize config account
    use pinocchio::sysvars::{rent::Rent, Sysvar};
    use pinocchio_system::instructions::Transfer;

    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(TokenLimitConfig::LEN);

    if config_account.lamports() < required_lamports {
        let lamports_needed = required_lamports - config_account.lamports();
        Transfer {
            from: payer,
            to: config_account,
            lamports: lamports_needed,
        }
        .invoke()?;
    }

    // Write config
    let config = TokenLimitConfig::new(
        *wallet_account.key(),
        0, // bump will be set by PDA derivation
        mint,
        initial_amount,
    );

    let config_data = unsafe { config_account.borrow_mut_data_unchecked() };
    if config_data.len() < TokenLimitConfig::LEN {
        return Err(ProgramError::InvalidAccountData);
    }

    let config_bytes = unsafe {
        core::slice::from_raw_parts(
            &config as *const TokenLimitConfig as *const u8,
            TokenLimitConfig::LEN,
        )
    };
    config_data[..TokenLimitConfig::LEN].copy_from_slice(config_bytes);

    // Set owner
    unsafe {
        config_account.assign(program_id);
    }

    Ok(())
}
