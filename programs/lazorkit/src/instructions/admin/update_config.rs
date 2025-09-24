use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, UpdateType},
};

/// Update program configuration settings
///
/// Allows the program authority to modify critical configuration parameters
/// including fee structures, default policy programs, and operational settings.
/// All fee updates are validated to ensure reasonable limits.
pub fn update_config(ctx: Context<UpdateConfig>, param: UpdateType, value: u64) -> Result<()> {
    let config = &mut ctx.accounts.config;

    match param {
        UpdateType::CreateWalletFee => {
            require!(value <= crate::constants::MAX_CREATE_WALLET_FEE, LazorKitError::InvalidFeeAmount);
            config.create_smart_wallet_fee = value;
        }
        UpdateType::FeePayerFee => {
            require!(value <= crate::constants::MAX_TRANSACTION_FEE, LazorKitError::InvalidFeeAmount);
            config.fee_payer_fee = value;
        }
        UpdateType::ReferralFee => {
            require!(value <= crate::constants::MAX_TRANSACTION_FEE, LazorKitError::InvalidFeeAmount);
            config.referral_fee = value;
        }
        UpdateType::LazorkitFee => {
            require!(value <= crate::constants::MAX_TRANSACTION_FEE, LazorKitError::InvalidFeeAmount);
            config.lazorkit_fee = value;
        }
        UpdateType::DefaultPolicyProgram => {
            // Get the new default policy program from remaining accounts
            let new_default_policy_program_info = ctx
                .remaining_accounts
                .first()
                .ok_or(LazorKitError::InvalidRemainingAccounts)?;

            // Ensure the new policy program is executable (not a data account)
            if !new_default_policy_program_info.executable {
                return err!(LazorKitError::ProgramNotExecutable);
            }

            // Update the default policy program ID for new wallets
            config.default_policy_program_id = new_default_policy_program_info.key();
        }
        UpdateType::Admin => {
            // Get the new admin authority from remaining accounts
            let new_admin_info = ctx
                .remaining_accounts
                .first()
                .ok_or(LazorKitError::InvalidRemainingAccounts)?;

            // Prevent setting system program or this program as admin (security measure)
            require!(
                new_admin_info.key() != anchor_lang::system_program::ID
                    && new_admin_info.key() != crate::ID,
                LazorKitError::InvalidAuthority
            );

            // Update the program authority
            config.authority = new_admin_info.key();
        }
        UpdateType::PauseProgram => {
            // Ensure program is not already paused
            require!(!config.is_paused, LazorKitError::ProgramPaused);
            config.is_paused = true;
        }
        UpdateType::UnpauseProgram => {
            // Ensure program is currently paused before unpausing
            require!(config.is_paused, LazorKitError::InvalidAccountState);
            config.is_paused = false;
        }
    }
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    /// The current authority of the program.
    #[account(
        mut,
        constraint = authority.key() == config.authority @ LazorKitError::AuthorityMismatch
    )]
    pub authority: Signer<'info>,

    /// The program's configuration account.
    #[account(
        mut,
        seeds = [Config::PREFIX_SEED],
        bump,
        has_one = authority @ LazorKitError::InvalidAuthority
    )]
    pub config: Box<Account<'info, Config>>,
}
