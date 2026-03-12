use assertions::sol_assert_bytes_eq;
use no_padding::NoPadding;
use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::find_program_address,
    pubkey::Pubkey, ProgramResult,
};

use crate::{error::AuthError, state::{config::ConfigAccount, AccountDiscriminator}};

/// Arguments for `UpdateConfig`.
/// Fixed length format: 53 bytes total.
/// - `update_wallet_fee`, `update_action_fee`, `update_num_shards`, `update_admin`, `num_shards` (5 bytes)
/// - `_padding` (3 bytes)
/// - `wallet_fee` (8 bytes)
/// - `action_fee` (8 bytes)
/// - `admin` (32 bytes)
#[repr(C, align(8))]
#[derive(Debug, NoPadding)]
pub struct UpdateConfigArgs {
    pub update_wallet_fee: u8,
    pub update_action_fee: u8,
    pub update_num_shards: u8,
    pub update_admin: u8,
    pub num_shards: u8,
    pub _padding: [u8; 3],
    pub wallet_fee: u64,
    pub action_fee: u64,
    pub admin: [u8; 32],
}

impl UpdateConfigArgs {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 56 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let update_wallet_fee = data[0];
        let update_action_fee = data[1];
        let update_num_shards = data[2];
        let update_admin = data[3];
        let num_shards = data[4];

        let mut wallet_fee_bytes = [0u8; 8];
        wallet_fee_bytes.copy_from_slice(&data[8..16]);
        let wallet_fee = u64::from_le_bytes(wallet_fee_bytes);

        let mut action_fee_bytes = [0u8; 8];
        action_fee_bytes.copy_from_slice(&data[16..24]);
        let action_fee = u64::from_le_bytes(action_fee_bytes);

        let mut admin = [0u8; 32];
        admin.copy_from_slice(&data[24..56]);

        Ok(Self {
            update_wallet_fee,
            update_action_fee,
            update_num_shards,
            update_admin,
            num_shards,
            _padding: [0; 3],
            wallet_fee,
            action_fee,
            admin,
        })
    }
}

/// Updates the global Config PDA settings.
///
/// Accounts:
/// 0. `[signer]` Admin (must match config.admin)
/// 1. `[writable]` Config PDA
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let args = UpdateConfigArgs::from_bytes(instruction_data)?;

    let account_info_iter = &mut accounts.iter();
    let admin_info = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let config_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !admin_info.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate Config PDA seeds
    let (config_key, _config_bump) = find_program_address(&[b"config"], program_id);
    if !sol_assert_bytes_eq(config_pda.key().as_ref(), config_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    let config_data = unsafe { config_pda.borrow_mut_data_unchecked() };
    if config_data.len() < std::mem::size_of::<ConfigAccount>() {
        return Err(ProgramError::UninitializedAccount);
    }

    // We can't use mutable reference to unaligned data easily without read_unaligned/write_unaligned
    let mut config_account =
        unsafe { std::ptr::read_unaligned(config_data.as_ptr() as *const ConfigAccount) };

    // Verify Admin
    if config_account.discriminator != AccountDiscriminator::Config as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if config_account.admin != *admin_info.key() {
        return Err(AuthError::PermissionDenied.into()); // Only current admin can update
    }

    // Apply updates
    if args.update_wallet_fee != 0 {
        config_account.wallet_fee = args.wallet_fee;
    }

    if args.update_action_fee != 0 {
        config_account.action_fee = args.action_fee;
    }

    if args.update_num_shards != 0 {
        if args.num_shards < config_account.num_shards {
            // Cannot decrease num_shards to avoid stranding funds
            return Err(ProgramError::InvalidArgument);
        }
        config_account.num_shards = args.num_shards;
    }

    if args.update_admin != 0 {
        config_account.admin = Pubkey::from(args.admin);
    }

    // Write back
    unsafe {
        std::ptr::write_unaligned(
            config_data.as_mut_ptr() as *mut ConfigAccount,
            config_account,
        );
    }

    Ok(())
}
