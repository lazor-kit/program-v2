//! Whitelist Plugin for LazorKit
//!
//! This plugin enforces address whitelisting for transfers.
//! Only allows transactions to pre-approved destination addresses.

use lazorkit_interface::{VerifyInstruction, INSTRUCTION_VERIFY};
use no_padding::NoPadding;
use pinocchio::{
    account_info::AccountInfo, entrypoint, msg, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

entrypoint!(process_instruction);

/// Maximum number of whitelisted addresses
pub const MAX_WHITELIST_SIZE: usize = 100;

/// Plugin state stored in the core wallet
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, NoPadding)]
pub struct WhitelistState {
    /// Number of whitelisted addresses
    pub count: u16,
    /// Padding for alignment
    pub _padding: [u8; 6],
    /// Whitelisted addresses (fixed size)
    pub addresses: [Pubkey; MAX_WHITELIST_SIZE],
}

impl WhitelistState {
    pub const LEN: usize = 2 + 6 + (32 * MAX_WHITELIST_SIZE);

    /// Unsafe load of mutable state from bytes (Zero-Copy)
    pub unsafe fn load_mut_unchecked(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(&mut *(data.as_mut_ptr() as *mut Self))
    }

    pub fn is_whitelisted(&self, address: &Pubkey) -> bool {
        // Simple linear scan (efficient enough for 100 items for now)
        for i in 0..self.count as usize {
            if &self.addresses[i] == address {
                return true;
            }
        }
        false
    }
}

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // 1. Parse instruction data (Zero-Copy)
    if instruction_data.len() < VerifyInstruction::LEN {
        msg!("Instruction data too short");
        return Err(ProgramError::InvalidInstructionData);
    }

    // Safety: VerifyInstruction is Pod
    let instruction = unsafe { &*(instruction_data.as_ptr() as *const VerifyInstruction) };

    // 2. Verify discriminator
    if instruction.discriminator != INSTRUCTION_VERIFY {
        msg!("Invalid instruction discriminator");
        return Err(ProgramError::InvalidInstructionData);
    }

    // 3. Get the account containing the state
    if accounts.is_empty() {
        msg!("No accounts provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let wallet_account = &accounts[0];

    // 4. Load state from offset
    let offset = instruction.state_offset as usize;
    let mut data = wallet_account.try_borrow_mut_data()?;

    if offset + WhitelistState::LEN > data.len() {
        msg!("Account data too small for state offset");
        return Err(ProgramError::AccountDataTooSmall);
    }

    let state = unsafe {
        WhitelistState::load_mut_unchecked(&mut data[offset..offset + WhitelistState::LEN])?
    };

    msg!(
        "Whitelist Plugin - Checking {} whitelisted addresses",
        state.count
    );

    // 5. Parse recipient from ORIGINAL execution data
    // execution_data follows VerifyInstruction in instruction_data
    let execution_data = &instruction_data[VerifyInstruction::LEN..];

    // Parse recipient using heuristic (same as before)
    if execution_data.len() < 32 {
        // Maybe it's not a transfer? If we can't parse recipient, what to do?
        // For security, if we can't verify, we should probably fail if this plugin is mandatory.
        // But maybe it's valid for non-transfer instructions?
        // Assuming this plugin intends to block UNKNOWN transfers.
        msg!("Instruction data too short to contain recipient");
        return Err(ProgramError::InvalidInstructionData);
    }

    let recipient_bytes: [u8; 32] = execution_data[0..32].try_into().unwrap();
    let recipient = Pubkey::from(recipient_bytes);

    msg!("Checking recipient: {:?}", recipient);

    if !state.is_whitelisted(&recipient) {
        msg!("Recipient {:?} is not in whitelist", recipient);
        // Fail
        return Err(ProgramError::Custom(1000)); // VerificationFailed
    }

    msg!("Whitelist check passed");
    Ok(())
}
