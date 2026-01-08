//! Remove Authority instruction handler - Pure External Architecture

use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
// Note: Using unsafe lamports manipulation instead of Transfer to avoid privilege escalation
use lazorkit_v2_assertions::check_self_owned;
use lazorkit_v2_state::{
    wallet_account::WalletAccount,
    position::Position,
    plugin_ref::PluginRef,
    Discriminator,
    Transmutable,
};

use crate::error::LazorkitError;

/// Arguments for RemoveAuthority instruction (Pure External)
/// Note: instruction discriminator is already parsed in process_action
#[repr(C, align(8))]
#[derive(Debug)]
pub struct RemoveAuthorityArgs {
    pub authority_id: u32,  // Authority ID to remove
    pub _padding: [u8; 4],
}

impl RemoveAuthorityArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for RemoveAuthorityArgs {
    const LEN: usize = Self::LEN;
}

/// Removes an authority from the wallet (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable)
/// 1. payer (writable, signer) - to receive refunded lamports
/// 2. system_program
/// 3..N. Additional accounts for authority authentication (signature, etc.)
pub fn remove_authority(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 3 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }
    
    let wallet_account_info = &accounts[0];
    let payer = &accounts[1];
    let system_program = &accounts[2];
    
    // Validate system program
    if system_program.key() != &pinocchio_system::ID {
        return Err(LazorkitError::InvalidSystemProgram.into());
    }
    
    // Validate wallet account
    check_self_owned(wallet_account_info, LazorkitError::OwnerMismatchWalletState)?;
    
    let wallet_account_data = unsafe { wallet_account_info.borrow_data_unchecked() };
    if wallet_account_data.is_empty() || wallet_account_data[0] != Discriminator::WalletAccount as u8 {
        return Err(LazorkitError::InvalidWalletStateDiscriminator.into());
    }
    
    let wallet_account = unsafe {
        WalletAccount::load_unchecked(&wallet_account_data[..WalletAccount::LEN])?
    };
    
    // Parse instruction args
    // Note: instruction discriminator (2 bytes) is already parsed in process_action
    if instruction_data.len() < RemoveAuthorityArgs::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    // Parse authority_id manually to avoid alignment issues
    let authority_id = u32::from_le_bytes([
        instruction_data[0],
        instruction_data[1],
        instruction_data[2],
        instruction_data[3],
    ]);
    
    // Get authority data to verify it exists
    let authority_data = wallet_account
        .get_authority(wallet_account_data, authority_id)?
        .ok_or(LazorkitError::InvalidAuthorityNotFoundByRoleId)?;
    
    // Authenticate with authority data (optional in Pure External - can be handled by plugin)
    // If authority_payload is provided in accounts[3], authenticate directly
    // Otherwise, skip authentication (plugins can handle it)
    let authority_payload = accounts.get(3).map(|acc| unsafe { acc.borrow_data_unchecked() });
    crate::util::authenticate::authenticate_authority(
        &authority_data,
        accounts,
        authority_payload,
        Some(instruction_data),
    )?;
    
    // Get current account size and number of authorities
    let current_size = wallet_account_data.len();
    let num_authorities = wallet_account.num_authorities(wallet_account_data)?;
    
    if num_authorities == 0 {
        return Err(LazorkitError::InvalidAuthorityNotFoundByRoleId.into());
    }
    
    // Find authority position and calculate removal
    let authorities_offset = wallet_account.authorities_offset();
    let mut authority_offset = authorities_offset;
    let mut found_authority = false;
    let mut authority_to_remove_size = 0usize;
    let mut authority_to_remove_start = 0usize;
    
    // First pass: find the authority to remove
    for _ in 0..num_authorities {
        if authority_offset + Position::LEN > current_size {
            break;
        }
        
        // Parse Position manually to avoid alignment issues
        let position_id = u32::from_le_bytes([
            wallet_account_data[authority_offset + 8],
            wallet_account_data[authority_offset + 9],
            wallet_account_data[authority_offset + 10],
            wallet_account_data[authority_offset + 11],
        ]);
        let position_boundary = u32::from_le_bytes([
            wallet_account_data[authority_offset + 12],
            wallet_account_data[authority_offset + 13],
            wallet_account_data[authority_offset + 14],
            wallet_account_data[authority_offset + 15],
        ]);
        
        if position_id == authority_id {
            found_authority = true;
            authority_to_remove_start = authority_offset;
            authority_to_remove_size = position_boundary as usize - authority_offset;
            break;
        }
        
        authority_offset = position_boundary as usize;
    }
    
    if !found_authority {
        return Err(LazorkitError::InvalidAuthorityNotFoundByRoleId.into());
    }
    
    // Calculate new account size
    let new_account_size = current_size - authority_to_remove_size;
    
    // Get mutable access
    let wallet_account_mut_data = unsafe { wallet_account_info.borrow_mut_data_unchecked() };
    
    // Compact data: shift all data after removed authority forward (like Swig)
    let data_after_removed = authority_to_remove_start + authority_to_remove_size;
    let remaining_len = current_size - data_after_removed;
    if remaining_len > 0 {
        // Shift data forward to fill the gap
        wallet_account_mut_data.copy_within(
            data_after_removed..data_after_removed + remaining_len,
            authority_to_remove_start,
        );
    }
    
    // Update boundaries of all authorities after the removed one
    // Need to adjust boundaries by subtracting authority_to_remove_size
    // (Following Swig pattern: update boundaries after shifting data)
    let mut cursor = authority_to_remove_start;
    let new_end = authority_to_remove_start + remaining_len;
    
    while cursor < new_end {
        if cursor + Position::LEN > new_end {
            break;
        }
        
        // Parse Position boundary
        let position_boundary = u32::from_le_bytes([
            wallet_account_mut_data[cursor + 12],
            wallet_account_mut_data[cursor + 13],
            wallet_account_mut_data[cursor + 14],
            wallet_account_mut_data[cursor + 15],
        ]);
        
        // Calculate and write the new boundary (subtract the removal size)
        if position_boundary as usize > authority_to_remove_size {
            let new_boundary = position_boundary.saturating_sub(authority_to_remove_size as u32);
            wallet_account_mut_data[cursor + 12..cursor + 16]
                .copy_from_slice(&new_boundary.to_le_bytes());
            cursor = new_boundary as usize;
        } else {
            // Invalid boundary, break to avoid infinite loop
            break;
        }
    }
    
    // Update num_authorities
    let new_num_authorities = num_authorities.saturating_sub(1);
    wallet_account.set_num_authorities(wallet_account_mut_data, new_num_authorities)?;
    
    // Resize account to new size
    let new_account_size_aligned = core::alloc::Layout::from_size_align(
        new_account_size,
        8,
    )
    .map_err(|_| LazorkitError::InvalidAlignment)?
    .pad_to_align()
    .size();
    
    wallet_account_info.resize(new_account_size_aligned)?;
    
    // Refund excess lamports to payer (using unsafe like Swig)
    let current_lamports = unsafe { *wallet_account_info.borrow_lamports_unchecked() };
    let required_lamports = Rent::get()?.minimum_balance(new_account_size_aligned);
    let excess_lamports = current_lamports.saturating_sub(required_lamports);
    
    if excess_lamports > 0 {
        unsafe {
            *wallet_account_info.borrow_mut_lamports_unchecked() = current_lamports - excess_lamports;
            *payer.borrow_mut_lamports_unchecked() = payer.lamports() + excess_lamports;
        }
    }
    
    Ok(())
}
