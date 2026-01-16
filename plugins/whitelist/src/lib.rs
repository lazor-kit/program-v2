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

    msg!("Whitelist Plugin - Checking whitelisted addresses");

    // 5. Verify ALL target accounts (index 2+) against whitelist
    // Accounts 0 (Config) and 1 (Vault) are trusted context provided by the Wallet Program.
    // Any other account passed to this instruction (which represents the target instruction's accounts)
    // MUST be in the whitelist. This includes the Target Program itself and any accounts it uses.

    if accounts.len() > 2 {
        for (_i, acc) in accounts[2..].iter().enumerate() {
            // Note: We check key() which returns &Pubkey
            if !state.is_whitelisted(acc.key()) {
                msg!("Account not in whitelist");
                return Err(ProgramError::Custom(1000)); // VerificationFailed
            }
        }
    } else {
        // If no target accounts are passed, it might be an empty instruction?
        // Or just checking the plugin itself?
        // Generally usually at least the target program is passed.
        // We permit "empty" target interactions if they don't touch any external accounts
        // (which is impossible for an invoke, but theoretically safe).
    }

    msg!("Whitelist check passed: All accounts verified");
    Ok(())
}
