use assertions::{check_zero_data, sol_assert_bytes_eq};
use no_padding::NoPadding;
use pinocchio::{
    account_info::AccountInfo, instruction::Seed, program_error::ProgramError,
    pubkey::find_program_address, pubkey::Pubkey, sysvars::rent::Rent, ProgramResult,
};

use crate::state::{config::ConfigAccount, AccountDiscriminator, CURRENT_ACCOUNT_VERSION};

/// Arguments for `InitializeConfig`.
/// Layout:
/// - `wallet_fee`: u64 (8 bytes)
/// - `action_fee`: u64 (8 bytes)
/// - `num_shards`: u8 (1 byte)
#[repr(C, align(8))]
#[derive(Debug, NoPadding)]
pub struct InitializeConfigArgs {
    pub wallet_fee: u64,
    pub action_fee: u64,
    pub num_shards: u8,
    pub _padding: [u8; 7],
}

impl InitializeConfigArgs {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 17 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let mut wallet_fee_bytes = [0u8; 8];
        wallet_fee_bytes.copy_from_slice(&data[0..8]);
        let wallet_fee = u64::from_le_bytes(wallet_fee_bytes);

        let mut action_fee_bytes = [0u8; 8];
        action_fee_bytes.copy_from_slice(&data[8..16]);
        let action_fee = u64::from_le_bytes(action_fee_bytes);

        let num_shards = data[16];
        if num_shards == 0 {
            return Err(ProgramError::InvalidInstructionData); // Must have at least 1 shard
        }

        Ok(Self {
            wallet_fee,
            action_fee,
            num_shards,
            _padding: [0; 7],
        })
    }
}

/// Initializes the global Config PDA.
///
/// Accounts:
/// 0. `[signer, writable]` Admin
/// 1. `[writable]` Config PDA
/// 2. `[]` System Program
/// 3. `[]` Rent Sysvar
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let args = InitializeConfigArgs::from_bytes(instruction_data)?;

    let account_info_iter = &mut accounts.iter();
    let admin_info = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let config_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let rent_sysvar = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !admin_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let rent = Rent::from_account_info(rent_sysvar)?;

    // Validate Config PDA seeds
    let (config_key, config_bump) = find_program_address(&[b"config"], program_id);
    if !sol_assert_bytes_eq(config_pda.key().as_ref(), config_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(config_pda, ProgramError::AccountAlreadyInitialized)?;

    // Initialize the Config PDA space
    let config_space = std::mem::size_of::<ConfigAccount>();
    let config_rent = rent.minimum_balance(config_space);

    let config_bump_arr = [config_bump];
    let config_seeds = [Seed::from(b"config"), Seed::from(&config_bump_arr)];

    crate::utils::initialize_pda_account(
        admin_info,
        config_pda,
        system_program,
        config_space,
        config_rent,
        program_id,
        &config_seeds,
    )?;

    // Write the data
    let config_data = unsafe { config_pda.borrow_mut_data_unchecked() };
    if (config_data.as_ptr() as usize) % 8 != 0 {
        return Err(ProgramError::InvalidAccountData);
    }

    let config_account = ConfigAccount {
        discriminator: AccountDiscriminator::Config as u8,
        bump: config_bump,
        version: CURRENT_ACCOUNT_VERSION,
        num_shards: args.num_shards,
        _padding: [0; 4],
        admin: *admin_info.key(),
        wallet_fee: args.wallet_fee,
        action_fee: args.action_fee,
    };

    unsafe {
        std::ptr::write_unaligned(
            config_data.as_mut_ptr() as *mut ConfigAccount,
            config_account,
        );
    }

    Ok(())
}
