use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

/// LazorKit SOL vault management utilities
/// 
/// Vaults are empty PDAs owned by the LazorKit program that hold SOL
/// for fee distribution and protocol operations. The system supports
/// up to 32 vault slots for efficient load distribution.
pub struct LazorKitVault;

impl LazorKitVault {
    pub const PREFIX_SEED: &'static [u8] = b"lazorkit_vault";
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

    /// Add SOL to vault by transferring from destination to vault
    pub fn add_sol<'info>(
        vault: &AccountInfo<'info>,
        destination: &AccountInfo<'info>,
        system_program: &Program<'info, System>,
        amount: u64,
    ) -> Result<()> {
        require!(
            amount >= crate::constants::EMPTY_PDA_RENT_EXEMPT_BALANCE,
            crate::error::LazorKitError::InsufficientBalanceForFee
        );

        transfer(
            CpiContext::new(
                system_program.to_account_info(),
                Transfer {
                    from: destination.to_account_info(),
                    to: vault.to_account_info(),
                },
            ),
            amount,
        )?;
        Ok(())
    }

    /// Remove SOL from vault by transferring from vault to destination
    pub fn remove_sol<'info>(
        vault: &AccountInfo<'info>,
        destination: &AccountInfo<'info>,
        system_program: &Program<'info, System>,
        amount: u64,
        index: u8,
        bump: u8,
    ) -> Result<()> {
        require!(
            vault.lamports() >= amount + crate::constants::EMPTY_PDA_RENT_EXEMPT_BALANCE,
            crate::error::LazorKitError::InsufficientVaultBalance
        );

        let seeds: &[&[u8]] = &[Self::PREFIX_SEED.as_ref(), &[index], &[bump]];
        let signer_seeds = &[&seeds[..]];

        transfer(
            CpiContext::new(
                system_program.to_account_info(),
                Transfer {
                    from: vault.to_account_info(),
                    to: destination.to_account_info(),
                },
            )
            .with_signer(signer_seeds),
            amount,
        )?;

        Ok(())
    }
}
