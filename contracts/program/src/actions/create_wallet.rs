//! CreateWallet instruction handler

use lazorkit_state::{
    authority::authority_type_to_length, vault_seeds_with_bump, wallet_seeds_with_bump,
    AuthorityType, LazorKitBuilder, LazorKitWallet, Position,
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

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
    let config_seeds = wallet_seeds_with_bump(&id, &bump_arr);
    let expected_config = pinocchio::pubkey::create_program_address(&config_seeds, program_id)
        .map_err(|_| ProgramError::InvalidSeeds)?;

    if config_account.key() != &expected_config {
        return Err(ProgramError::InvalidSeeds);
    }

    // Verify Vault PDA
    let vault_bump_arr = [wallet_bump];
    let vault_seeds = vault_seeds_with_bump(&expected_config, &vault_bump_arr);
    let expected_vault = pinocchio::pubkey::create_program_address(&vault_seeds, program_id)
        .map_err(|_| ProgramError::InvalidSeeds)?;

    if vault_account.key() != &expected_vault {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate authority type
    let auth_type = AuthorityType::try_from(owner_authority_type)?;
    let auth_len = authority_type_to_length(&auth_type)?;

    // Calculate exact space needed: Wallet header + Position + Authority only
    // No plugin buffer - account will be reallocated when plugins are added later
    let initial_role_size = Position::LEN + auth_len;
    let space = LazorKitWallet::LEN + initial_role_size;
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(space);

    // Create Config account using the seeds we already validated
    let config_seed_list = [
        Seed::from(config_seeds[0]),
        Seed::from(config_seeds[1]),
        Seed::from(config_seeds[2]),
    ];
    let config_signer = pinocchio::instruction::Signer::from(&config_seed_list);

    CreateAccount {
        from: payer_account,
        to: config_account,
        lamports,
        space: space as u64,
        owner: program_id,
    }
    .invoke_signed(&[config_signer])?;

    // Create Vault PDA account
    // This is a System Program-owned account (no data storage needed)
    // Used as the wallet address for holding SOL and SPL tokens
    // The vault is controlled by the config PDA through signature verification

    let vault_seed_list = [
        Seed::from(vault_seeds[0]),
        Seed::from(vault_seeds[1]),
        Seed::from(vault_seeds[2]),
    ];
    let vault_signer = pinocchio::instruction::Signer::from(&vault_seed_list);

    CreateAccount {
        from: payer_account,
        to: vault_account,
        lamports: rent.minimum_balance(0),
        space: 0,
        owner: &pinocchio_system::ID,
    }
    .invoke_signed(&[vault_signer])?;

    // Initialize wallet configuration using builder pattern
    // This handles zero-copy serialization of wallet header and role data
    let config_data = unsafe { config_account.borrow_mut_data_unchecked() };
    let wallet = LazorKitWallet::new(id, bump, wallet_bump);
    let mut wallet_builder = LazorKitBuilder::create(config_data, wallet)?;

    msg!("LazorKit wallet created:");
    msg!("  Config: {:?}", config_account.key());
    msg!("  Vault: {:?}", vault_account.key());
    msg!("  Owner Authority Type: {:?}", auth_type);

    // Add initial owner role with authority data
    // Empty actions array means no plugins are attached to this role initially
    wallet_builder.add_role(auth_type, &owner_authority_data)?;

    Ok(())
}
