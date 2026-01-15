//! LazorKit Program - Main Entry Point
//!
//! Modular smart wallet protocol with pluggable validation logic.

extern crate alloc;

pub mod actions;
pub mod error;
pub mod instruction;
pub mod processor;

use core::mem::MaybeUninit;
use pinocchio::{
    account_info::AccountInfo,
    lazy_entrypoint::{InstructionContext, MaybeAccount},
    lazy_program_entrypoint,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};
use pinocchio_pubkey::declare_id;

declare_id!("LazorKit11111111111111111111111111111111111");

lazy_program_entrypoint!(process_instruction);

fn process_instruction(mut ctx: InstructionContext) -> ProgramResult {
    // Collect accounts into a stack array
    // We assume a reasonable max accounts
    const MAX_ACCOUNTS: usize = 64;
    const AI: MaybeUninit<AccountInfo> = MaybeUninit::<AccountInfo>::uninit();
    let mut accounts_storage = [AI; MAX_ACCOUNTS];
    let mut accounts_len = 0;

    while let Ok(acc) = ctx.next_account() {
        if accounts_len >= MAX_ACCOUNTS {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        match acc {
            MaybeAccount::Account(account) => {
                accounts_storage[accounts_len].write(account);
            },
            MaybeAccount::Duplicated(idx) => {
                // Pinocchio optimization: duplicated account references a previous index
                let original = unsafe { accounts_storage[idx as usize].assume_init_ref().clone() };
                accounts_storage[accounts_len].write(original);
            },
        }
        accounts_len += 1;
    }

    // Create slice from initialized accounts
    let accounts = unsafe {
        core::slice::from_raw_parts(
            accounts_storage.as_ptr() as *const AccountInfo,
            accounts_len,
        )
    };

    // Get instruction data
    let instruction_data = unsafe { ctx.instruction_data_unchecked() };

    // Delegate to processor
    // Pinocchio doesn't pass program_id dynamically in InstructionContext,
    // so we pass our own ID (checks should verify program_id against this if needed).
    processor::process_instruction(&crate::ID, accounts, instruction_data)
}
