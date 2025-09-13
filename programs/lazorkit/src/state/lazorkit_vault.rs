use anchor_lang::prelude::*;

/// Utility functions for LazorKit SOL vaults
/// Vaults are empty PDAs owned by the LazorKit program that hold SOL
pub struct LazorKitVault;

impl LazorKitVault {
    pub const PREFIX_SEED: &'static [u8] = b"vault";
    pub const MAX_VAULTS: u8 = 32;

    /// Derive vault PDA for a given index
    pub fn derive_vault_address(index: u8, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[Self::PREFIX_SEED, &index.to_le_bytes()], program_id)
    }

    /// Validate that the provided vault account matches the expected vault for the given index
    pub fn validate_vault_for_index(
        vault_account: &Pubkey,
        vault_index: u8,
        program_id: &Pubkey,
    ) -> Result<()> {
        require!(
            vault_index < Self::MAX_VAULTS,
            crate::error::LazorKitError::InvalidVaultIndex
        );

        let (expected_vault, _) = Self::derive_vault_address(vault_index, program_id);

        require!(
            *vault_account == expected_vault,
            crate::error::LazorKitError::InvalidVaultIndex
        );

        Ok(())
    }

    /// Get the current SOL balance of a vault account
    pub fn get_sol_balance(vault_account: &AccountInfo) -> u64 {
        vault_account.lamports()
    }

    /// Remove SOL from vault by transferring from vault to destination
    pub fn remove_sol(vault: &AccountInfo, destination: &AccountInfo, amount: u64) -> Result<()> {
        require!(
            vault.lamports() >= amount,
            crate::error::LazorKitError::InsufficientVaultBalance
        );

        **vault.try_borrow_mut_lamports()? -= amount;
        **destination.try_borrow_mut_lamports()? += amount;

        Ok(())
    }
}
