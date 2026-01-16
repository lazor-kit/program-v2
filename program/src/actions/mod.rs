pub mod add_authority;
pub mod create_session;
pub mod create_wallet;
pub mod deactivate_policy;
pub mod execute;
pub mod register_policy;
pub mod remove_authority;
pub mod transfer_ownership;
pub mod update_authority;

pub use add_authority::*;
pub use create_session::*;
pub use create_wallet::*;
pub use deactivate_policy::*;
pub use execute::*;
pub use register_policy::*;
pub use remove_authority::*;
pub use transfer_ownership::*;
pub use update_authority::*;

use crate::error::LazorKitError;
use lazorkit_state::registry::PolicyRegistryEntry;
use pinocchio::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};

pub fn verify_policy_registry(
    program_id: &Pubkey,
    policy_program_id: &Pubkey,
    registry_accounts: &[AccountInfo],
) -> Result<(), ProgramError> {
    let (expected_pda, _) = pinocchio::pubkey::find_program_address(
        &[PolicyRegistryEntry::SEED_PREFIX, policy_program_id.as_ref()],
        program_id,
    );

    let registry_acc = registry_accounts
        .iter()
        .find(|acc| acc.key() == &expected_pda)
        .ok_or_else(|| {
            msg!(
                "Registry account not provided for policy {:?}",
                policy_program_id
            );
            LazorKitError::UnverifiedPolicy // TODO: Rename Error Variant
        })?;

    if registry_acc.data_is_empty() {
        msg!("Registry account empty for policy {:?}", policy_program_id);
        return Err(LazorKitError::UnverifiedPolicy.into());
    }

    let data = registry_acc.try_borrow_data()?;
    if data.len() < PolicyRegistryEntry::LEN {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify program_id matches (offset 16..48)
    if &data[16..48] != policy_program_id.as_ref() {
        msg!("Registry program_id mismatch");
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify is_active (offset 48)
    let is_active = data[48] != 0;
    if !is_active {
        msg!("Policy deactivated: {:?}", policy_program_id);
        return Err(LazorKitError::PolicyDeactivated.into()); // TODO: Rename Error Variant
    }

    Ok(())
}
