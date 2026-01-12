//! Lazorkit V2 Program Implementation
//!
//! This module provides the core program implementation for the Lazorkit V2 wallet
//! system. It handles instruction processing and program state management.

pub mod actions;
mod error;
pub mod instruction;
pub mod util;

use actions::process_action;
use error::LazorkitError;
use lazorkit_v2_state::{wallet_account::WalletAccount, Discriminator};
#[cfg(not(feature = "no-entrypoint"))]
use pinocchio::lazy_program_entrypoint;
use pinocchio::{
    account_info::AccountInfo,
    lazy_entrypoint::{InstructionContext, MaybeAccount},
    program_error::ProgramError,
    ProgramResult,
};
use pinocchio_pubkey::declare_id;
#[cfg(not(feature = "no-entrypoint"))]
use {default_env::default_env, solana_security_txt::security_txt};

declare_id!("CmF46cm89WjdfCDDDTx5X2kQLc2mFVUhP3k7k3txgAFE");

pinocchio::default_allocator!();
pinocchio::default_panic_handler!();

#[cfg(target_os = "solana")]
use getrandom::{register_custom_getrandom, Error};

#[cfg(target_os = "solana")]
pub fn custom_getrandom(_buf: &mut [u8]) -> Result<(), Error> {
    panic!("getrandom not supported on solana");
}

#[cfg(target_os = "solana")]
register_custom_getrandom!(custom_getrandom);

// Manual entrypoint implementation to avoid `pinocchio::entrypoint!` macro issues
// which can cause "Entrypoint out of bounds" errors due to `cfg` attributes or
// excessive stack allocation (MAX_ACCOUNTS).
// We manually allocate a smaller buffer (32 accounts) to keep stack usage safe (SBF stack is 4KB).
#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    const MAX_ACCOUNTS: usize = 32;
    let mut accounts_buffer = [core::mem::MaybeUninit::<AccountInfo>::uninit(); MAX_ACCOUNTS];
    let (program_id, num_accounts, instruction_data) =
        pinocchio::entrypoint::deserialize(input, &mut accounts_buffer);
    let accounts =
        core::slice::from_raw_parts(accounts_buffer.as_ptr() as *const AccountInfo, num_accounts);
    match process_instruction(&program_id, accounts, &instruction_data) {
        Ok(()) => pinocchio::SUCCESS,
        Err(e) => e.into(),
    }
}

pub fn process_instruction(
    _program_id: &pinocchio::pubkey::Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    process_action(accounts, instruction_data)
}

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Lazorkit V2",
    project_url: "https://lazorkit.com",
    contacts: "email:security@lazorkit.com",
    policy: "https://github.com/lazorkit/lazorkit-v2/security/policy",
    preferred_languages: "en",
    source_code: "https://github.com/lazorkit/lazorkit-v2"
}
