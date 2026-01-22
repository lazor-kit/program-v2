//! CreateWallet instruction handler

use lazorkit_state::{
    authority::authority_type_to_length, vault_seeds_with_bump, wallet_seeds_with_bump,
    AuthorityType, LazorKitBuilder, LazorKitWallet, Position,
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::{CreateAccount, Transfer};

pub fn process_create_wallet(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    id: [u8; 32],
    bump: u8,
    wallet_bump: u8,
    owner_authority_type: u16,
    owner_authority_data: Vec<u8>,
) -> ProgramResult {
    let mut account_info_iter = accounts.iter();
    let config_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let payer_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let vault_account = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // Verify signer
    if !payer_account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify Config PDA
    let bump_arr = [bump];
    // wallet_seeds_with_bump returns [&[u8]; 3] but we need seeds for validation
    let config_seeds = wallet_seeds_with_bump(&id, &bump_arr);

    // Check derivation logic
    let expected_config = pinocchio::pubkey::create_program_address(&config_seeds, program_id)
        .map_err(|_| {
            msg!("Error: create_program_address failed (On Curve)");
            ProgramError::InvalidSeeds
        })?;

    if config_account.key() != &expected_config {
        msg!("Error: Config PDA mismatch!");
        return Err(ProgramError::InvalidSeeds);
    }

    // Verify Vault PDA
    let vault_bump_arr = [wallet_bump];
    let vault_seeds = vault_seeds_with_bump(&expected_config, &vault_bump_arr);
    let expected_vault = pinocchio::pubkey::create_program_address(&vault_seeds, program_id)
        .map_err(|_| ProgramError::InvalidSeeds)?;

    if vault_account.key() != &expected_vault {
        msg!("Error: Vault PDA mismatch!");
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate authority
    let auth_type = AuthorityType::try_from(owner_authority_type)?;
    let auth_len = authority_type_to_length(&auth_type)?;

    let initial_role_size = Position::LEN + auth_len;
    let space = LazorKitWallet::LEN + initial_role_size;
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(space);

    // Create Config Account
    msg!("Creating Config via pinocchio-system");

    // Construct signer seeds
    let config_signer_seeds = [
        Seed::from(b"lazorkit"),
        Seed::from(id.as_slice()),
        Seed::from(bump_arr.as_slice()),
    ];

    // Check if account already exists/has lamports
    let current_lamports = unsafe { *config_account.borrow_lamports_unchecked() };
    let lamports_to_transfer = if current_lamports >= lamports {
        0
    } else {
        lamports - current_lamports
    };

    if lamports_to_transfer > 0 {
        CreateAccount {
            from: payer_account,
            to: config_account,
            lamports: lamports_to_transfer,
            space: space as u64,
            owner: program_id,
        }
        .invoke_signed(&[Signer::from(&config_signer_seeds)])?;
    } else {
        // If it already has lamports but we want to allocate space/owner
        // Note: CreateAccount usually handles everything. If we are just responding to "lamports > 0"
        // we might miss allocating space if it was pre-funded but not created.
        // Swig logic: check_zero_data enforces it's empty.
        // Here we just proceed. CreateAccount instruction in system program fails if account exists with different owner.
        // Assuming standard flow where it's a new PDA.
        CreateAccount {
            from: payer_account,
            to: config_account,
            lamports: 0, // No extra lamports needed
            space: space as u64,
            owner: program_id,
        }
        .invoke_signed(&[Signer::from(&config_signer_seeds)])?;
    }

    // Create Vault (Owner = System Program)
    // Swig uses Transfer to create system accounts
    msg!("Creating Vault via pinocchio-system Transfer");
    let vault_rent = rent.minimum_balance(0);
    let vault_lamports = unsafe { *vault_account.borrow_lamports_unchecked() };

    let vault_transfer_amount = if vault_lamports >= vault_rent {
        0
    } else {
        vault_rent - vault_lamports
    };

    if vault_transfer_amount > 0 {
        Transfer {
            from: payer_account,
            to: vault_account,
            lamports: vault_transfer_amount,
        }
        .invoke()?;
    }

    // Initialize wallet
    let config_data = unsafe { config_account.borrow_mut_data_unchecked() };
    let wallet = LazorKitWallet::new(id, bump, wallet_bump);
    let mut wallet_builder = LazorKitBuilder::create(config_data, wallet)?;

    msg!("LazorKit wallet created successfully");
    wallet_builder.add_role(auth_type, &owner_authority_data, 0)?;

    Ok(())
}
