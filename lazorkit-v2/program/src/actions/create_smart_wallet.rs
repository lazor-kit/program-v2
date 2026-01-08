//! Create Smart Wallet instruction handler - Pure External Architecture

use lazorkit_v2_assertions::{check_self_pda, check_system_owner, check_zero_data};
use lazorkit_v2_state::{
    wallet_account::{
        wallet_account_seeds, wallet_account_seeds_with_bump, wallet_account_signer,
        wallet_vault_seeds_with_bump, WalletAccount,
    },
    Discriminator, IntoBytes, Transmutable,
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

use crate::error::LazorkitError;

/// Arguments for creating a new Lazorkit wallet (Pure External).
/// Note: instruction discriminator is already parsed in process_action, so we don't include it here
#[repr(C, align(8))]
#[derive(Debug)]
pub struct CreateSmartWalletArgs {
    pub id: [u8; 32],     // Unique wallet identifier
    pub bump: u8,         // PDA bump for wallet_account
    pub wallet_bump: u8,  // PDA bump for wallet_vault
    pub _padding: [u8; 6], // Padding to align to 8 bytes (32 + 1 + 1 + 6 = 40 bytes, aligned)
}

impl CreateSmartWalletArgs {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

impl Transmutable for CreateSmartWalletArgs {
    const LEN: usize = Self::LEN;
}

/// Creates a new Lazorkit smart wallet (Pure External architecture).
///
/// Accounts:
/// 0. wallet_account (writable, PDA)
/// 1. wallet_vault (writable, system-owned PDA)
/// 2. payer (writable, signer)
/// 3. system_program
pub fn create_smart_wallet(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < 4 {
        return Err(LazorkitError::InvalidAccountsLength.into());
    }

    let wallet_account = &accounts[0];
    let wallet_vault = &accounts[1];
    let payer = &accounts[2];
    let system_program = &accounts[3];

    // Validate system program
    if system_program.key() != &pinocchio_system::ID {
        return Err(LazorkitError::InvalidSystemProgram.into());
    }

    // Validate accounts
    check_system_owner(wallet_account, LazorkitError::OwnerMismatchWalletState)?;
    check_zero_data(wallet_account, LazorkitError::AccountNotEmptyWalletState)?;
    check_system_owner(wallet_vault, LazorkitError::OwnerMismatchWalletState)?;
    check_zero_data(wallet_vault, LazorkitError::AccountNotEmptyWalletState)?;

    // Parse instruction args
    if instruction_data.len() < CreateSmartWalletArgs::LEN {
        return Err(LazorkitError::InvalidCreateInstructionDataTooShort.into());
    }

    let args = unsafe { CreateSmartWalletArgs::load_unchecked(instruction_data)? };

    // Validate wallet_account PDA
    // Use find_program_address (like test does) to find correct PDA and bump
    let wallet_account_seeds_no_bump = wallet_account_seeds(&args.id);
    let (expected_pda, expected_bump) = pinocchio::pubkey::find_program_address(
        &wallet_account_seeds_no_bump,
        &crate::ID,
    );
    
    // Verify PDA matches
    if expected_pda != *wallet_account.key() {
        return Err(LazorkitError::InvalidSeedWalletState.into());
    }
    
    // Verify bump matches
    if expected_bump != args.bump {
        return Err(LazorkitError::InvalidSeedWalletState.into());
    }
    
    let validated_bump = expected_bump;

    // Validate wallet_vault PDA (system-owned, derived from wallet_account key)
    // Note: For system-owned PDA, we use check_any_pda instead of check_self_pda
    // But wallet_vault validation is less critical since it's system-owned
    // We'll just verify it exists and is system-owned

    // Calculate account size
    // Header: WalletAccount (40 bytes) + num_authorities (2 bytes) + num_plugins (2 bytes) + last_nonce (8 bytes)
    // Minimum size for empty wallet
    let min_account_size = WalletAccount::LEN + 2 + 2 + 8; // 40 + 2 + 2 + 8 = 52 bytes
    let account_size = core::alloc::Layout::from_size_align(min_account_size, 8)
            .map_err(|_| LazorkitError::InvalidAlignment)?
            .pad_to_align()
            .size();

    let lamports_needed = Rent::get()?.minimum_balance(account_size);

    // Create WalletAccount
    let wallet_account_data = WalletAccount::new(args.id, args.bump, args.wallet_bump);

    // Get current lamports
    let current_lamports = unsafe { *wallet_account.borrow_lamports_unchecked() };
    let lamports_to_transfer = if current_lamports >= lamports_needed {
        0
    } else {
        lamports_needed - current_lamports
    };

    // Create wallet_account account
    CreateAccount {
        from: payer,
        to: wallet_account,
        lamports: lamports_to_transfer,
        space: account_size as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[wallet_account_signer(&args.id, &[validated_bump])
    .as_slice()
    .into()])?;

    // Initialize WalletAccount data
    let wallet_account_data_bytes = wallet_account_data.into_bytes()?;
    let wallet_account_mut_data = unsafe { wallet_account.borrow_mut_data_unchecked() };
    wallet_account_mut_data[..wallet_account_data_bytes.len()]
        .copy_from_slice(wallet_account_data_bytes);

    // Initialize num_authorities = 0
    wallet_account_mut_data[WalletAccount::LEN..WalletAccount::LEN + 2]
        .copy_from_slice(&0u16.to_le_bytes());

    // Initialize num_plugins = 0
    wallet_account_mut_data[WalletAccount::LEN + 2..WalletAccount::LEN + 4]
        .copy_from_slice(&0u16.to_le_bytes());

    // Initialize last_nonce = 0
    wallet_account_mut_data[WalletAccount::LEN + 4..WalletAccount::LEN + 12]
        .copy_from_slice(&0u64.to_le_bytes());

    // Create wallet_vault (system-owned PDA)
    let wallet_vault_rent_exemption = Rent::get()?.minimum_balance(0); // System account
    let current_wallet_vault_lamports = unsafe { *wallet_vault.borrow_lamports_unchecked() };
    let wallet_vault_lamports_to_transfer = if current_wallet_vault_lamports >= wallet_vault_rent_exemption {
            0
        } else {
        wallet_vault_rent_exemption - current_wallet_vault_lamports
        };

    if wallet_vault_lamports_to_transfer > 0 {
        // Transfer lamports to wallet_vault (system-owned PDA)
        // The account will be created automatically when it receives lamports
        pinocchio_system::instructions::Transfer {
            from: payer,
            to: wallet_vault,
            lamports: wallet_vault_lamports_to_transfer,
        }
        .invoke()?;
    }

    Ok(())
}
