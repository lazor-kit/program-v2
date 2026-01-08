//! Lazorkit V2 Program Implementation
//!
//! This module provides the core program implementation for the Lazorkit V2 wallet
//! system. It handles account classification, instruction processing, and
//! program state management.

pub mod actions;
mod error;
pub mod instruction;
pub mod util;

use actions::process_action;
use error::LazorkitError;
#[cfg(not(feature = "no-entrypoint"))]
use pinocchio::lazy_program_entrypoint;
use pinocchio::{
    account_info::AccountInfo,
    lazy_entrypoint::{InstructionContext, MaybeAccount},
    memory::sol_memcmp,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};
use pinocchio_pubkey::{declare_id, pubkey};
use lazorkit_v2_state::{AccountClassification, Discriminator, wallet_account::WalletAccount};
#[cfg(not(feature = "no-entrypoint"))]
use {default_env::default_env, solana_security_txt::security_txt};

declare_id!("Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh");

/// Program ID for the SPL Token program
const SPL_TOKEN_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
/// Program ID for the SPL Token 2022 program
const SPL_TOKEN_2022_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
/// Program ID for the Solana Staking program
const STAKING_ID: Pubkey = pubkey!("Stake11111111111111111111111111111111111111");
/// Program ID for the Solana System program
const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");

pinocchio::default_allocator!();
pinocchio::default_panic_handler!();

#[cfg(not(feature = "no-entrypoint"))]
lazy_program_entrypoint!(process_instruction);

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Lazorkit V2",
    project_url: "https://lazorkit.com",
    contacts: "email:security@lazorkit.com",
    policy: "https://github.com/lazorkit/lazorkit-v2/security/policy",
    preferred_languages: "en",
    source_code: "https://github.com/lazorkit/lazorkit-v2"
}

/// Main program entry point.
///
/// This function is called by the Solana runtime to process instructions sent
/// to the Lazorkit V2 program. It sets up the execution context and delegates
/// to the `execute` function for actual instruction processing.
pub fn process_instruction(mut ctx: InstructionContext) -> ProgramResult {
    use lazorkit_v2_instructions::MAX_ACCOUNTS;
    const AI: core::mem::MaybeUninit<AccountInfo> = core::mem::MaybeUninit::<AccountInfo>::uninit();
    const AC: core::mem::MaybeUninit<AccountClassification> = core::mem::MaybeUninit::<AccountClassification>::uninit();
    let mut accounts = [AI; MAX_ACCOUNTS];
    let mut classifiers = [AC; MAX_ACCOUNTS];
    unsafe {
        execute(&mut ctx, &mut accounts, &mut classifiers)?;
    }
    Ok(())
}

/// Core instruction execution function.
///
/// This function processes all accounts in the instruction context, classifies
/// them according to their type and ownership, and then processes the
/// instruction action.
#[inline(always)]
unsafe fn execute(
    ctx: &mut InstructionContext,
    accounts: &mut [core::mem::MaybeUninit<AccountInfo>],
    account_classification: &mut [core::mem::MaybeUninit<AccountClassification>],
) -> Result<(), ProgramError> {
    let mut index: usize = 0;

    // First account must be processed to get WalletState
    if let Ok(acc) = ctx.next_account() {
        match acc {
            MaybeAccount::Account(account) => {
                let classification =
                    classify_account(0, &account, accounts, account_classification, None)?;
                account_classification[0].write(classification);
                accounts[0].write(account);
            },
            MaybeAccount::Duplicated(account_index) => {
                accounts[0].write(accounts[account_index as usize].assume_init_ref().clone());
            },
        }
        index = 1;
    }

    // Process remaining accounts
    while let Ok(acc) = ctx.next_account() {
        let classification = match &acc {
            MaybeAccount::Account(account) => classify_account(
                index,
                account,
                accounts,
                account_classification,
                None,
            )?,
            MaybeAccount::Duplicated(account_index) => {
                let account = accounts[*account_index as usize].assume_init_ref().clone();
                classify_account(
                    index,
                    &account,
                    accounts,
                    account_classification,
                    None,
                )?
            },
        };
        account_classification[index].write(classification);
        accounts[index].write(match acc {
            MaybeAccount::Account(account) => account,
            MaybeAccount::Duplicated(account_index) => {
                accounts[account_index as usize].assume_init_ref().clone()
            },
        });
        index += 1;
    }

    // Dispatch to action handler
    process_action(
        core::slice::from_raw_parts(accounts.as_ptr() as _, index),
        core::slice::from_raw_parts_mut(account_classification.as_mut_ptr() as _, index),
        ctx.instruction_data_unchecked(),
    )?;
    Ok(())
}

/// Classifies an account based on its owner and data.
///
/// This function determines the type and role of an account in the Lazorkit V2 wallet
/// system. It handles several special cases:
/// - Lazorkit accounts (the first one must be at index 0)
/// - Stake accounts (with validation of withdrawer authority)
/// - Token accounts (SPL Token and Token-2022)
#[inline(always)]
unsafe fn classify_account(
    index: usize,
    account: &AccountInfo,
    accounts: &[core::mem::MaybeUninit<AccountInfo>],
    account_classifications: &[core::mem::MaybeUninit<AccountClassification>],
    _program_scope_cache: Option<&()>,
) -> Result<AccountClassification, ProgramError> {
    match account.owner() {
        &crate::ID => {
            let data = account.borrow_data_unchecked();
            let first_byte = *data.get_unchecked(0);
            match first_byte {
                disc if disc == Discriminator::WalletAccount as u8 && index == 0 => {
                    Ok(AccountClassification::ThisLazorkitConfig {
                        lamports: account.lamports(),
                    })
                },
                disc if disc == Discriminator::WalletAccount as u8 && index != 0 => {
                    let first_account = accounts.get_unchecked(0).assume_init_ref();
                    let first_data = first_account.borrow_data_unchecked();

                    if first_account.owner() == &crate::ID
                        && first_data.len() >= 8
                        && *first_data.get_unchecked(0) == Discriminator::WalletAccount as u8
                    {
                        Ok(AccountClassification::None)
                    } else {
                        Err(LazorkitError::InvalidAccountsWalletStateMustBeFirst.into())
                    }
                },
                _ => Ok(AccountClassification::None),
            }
        },
        &SYSTEM_PROGRAM_ID if index == 1 => {
            let first_account = accounts.get_unchecked(0).assume_init_ref();
            let first_data = first_account.borrow_data_unchecked();

            if first_account.owner() == &crate::ID
                && first_data.len() >= 8
                && *first_data.get_unchecked(0) == Discriminator::WalletAccount as u8
            {
                return Ok(AccountClassification::LazorkitWalletAddress {
                    lamports: first_account.lamports(),
                });
            }
            Ok(AccountClassification::None)
        },
        &STAKING_ID => {
            let data = account.borrow_data_unchecked();
            if data.len() >= 200 && index > 0 {
                let authorized_withdrawer = unsafe { data.get_unchecked(44..76) };

                if sol_memcmp(
                    accounts.get_unchecked(0).assume_init_ref().key(),
                    authorized_withdrawer,
                    32,
                ) == 0
                {
                    let state_value = u32::from_le_bytes(
                        data.get_unchecked(196..200)
                            .try_into()
                            .map_err(|_| ProgramError::InvalidAccountData)?,
                    );

                    let stake_amount = u64::from_le_bytes(
                        data.get_unchecked(184..192)
                            .try_into()
                            .map_err(|_| ProgramError::InvalidAccountData)?,
                    );

                    return Ok(AccountClassification::LazorkitStakeAccount {
                        balance: stake_amount,
                    });
                }
            }
            Ok(AccountClassification::None)
        },
        &SPL_TOKEN_2022_ID | &SPL_TOKEN_ID if account.data_len() >= 165 && index > 0 => unsafe {
            let data = account.borrow_data_unchecked();
            let token_authority = data.get_unchecked(32..64);

            let matches_lazorkit_account = sol_memcmp(
                accounts.get_unchecked(0).assume_init_ref().key(),
                token_authority,
                32,
            ) == 0;

            let matches_lazorkit_wallet_address = if index > 1 {
                if matches!(
                    account_classifications.get_unchecked(1).assume_init_ref(),
                    AccountClassification::LazorkitWalletAddress { .. }
                ) {
                    sol_memcmp(
                        accounts.get_unchecked(1).assume_init_ref().key(),
                        token_authority,
                        32,
                    ) == 0
                } else {
                    false
                }
            } else {
                false
            };

            if matches_lazorkit_account || matches_lazorkit_wallet_address {
                let mint_bytes: [u8; 32] = data
                    .get_unchecked(0..32)
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?;
                let mint = Pubkey::from(mint_bytes);

                let owner_bytes: [u8; 32] = data
                    .get_unchecked(32..64)
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?;
                let owner = Pubkey::from(owner_bytes);
                Ok(AccountClassification::LazorkitTokenAccount {
                    owner,
                    mint,
                    amount: u64::from_le_bytes(
                        data.get_unchecked(64..72)
                            .try_into()
                            .map_err(|_| ProgramError::InvalidAccountData)?,
                    ),
                })
            } else {
                Ok(AccountClassification::None)
            }
        },
        _ => Ok(AccountClassification::None),
    }
}
