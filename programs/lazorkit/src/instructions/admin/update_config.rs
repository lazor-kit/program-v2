use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{UpdateType, ProgramConfig},
};

pub fn update_config(
    ctx: Context<UpdateConfig>,
    param: UpdateType,
    value: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    match param {
        UpdateType::CreateWalletFee => {
            // Validate fee is reasonable (max 1 SOL)
            require!(value <= 1_000_000_000, LazorKitError::InvalidFeeAmount);
            config.create_smart_wallet_fee = value;
        }
        UpdateType::FeePayerFee => {
            // Validate fee is reasonable (max 0.1 SOL)
            require!(value <= 100_000_000, LazorKitError::InvalidFeeAmount);
            config.fee_payer_fee = value;
        }
        UpdateType::ReferralFee => {
            // Validate fee is reasonable (max 0.1 SOL)
            require!(value <= 100_000_000, LazorKitError::InvalidFeeAmount);
            config.referral_fee = value;
        }
        UpdateType::LazorkitFee => {
            // Validate fee is reasonable (max 0.1 SOL)
            require!(value <= 100_000_000, LazorKitError::InvalidFeeAmount);
            config.lazorkit_fee = value;
        }
        UpdateType::DefaultPolicyProgram => {
            let new_default_policy_program_info = ctx
                .remaining_accounts
                .first()
                .ok_or(LazorKitError::InvalidRemainingAccounts)?;

            // Check if the new default policy program is executable
            if !new_default_policy_program_info.executable {
                return err!(LazorKitError::ProgramNotExecutable);
            }

            config.default_policy_program_id = new_default_policy_program_info.key();
        }
        UpdateType::Admin => {
            let new_admin_info = ctx
                .remaining_accounts
                .first()
                .ok_or(LazorKitError::InvalidRemainingAccounts)?;

            // Cannot set admin to system program or this program
            require!(
                new_admin_info.key() != anchor_lang::system_program::ID
                    && new_admin_info.key() != crate::ID,
                LazorKitError::InvalidAuthority
            );

            config.authority = new_admin_info.key();
        }
        UpdateType::PauseProgram => {
            require!(!config.is_paused, LazorKitError::ProgramPaused);
            config.is_paused = true;
        }
        UpdateType::UnpauseProgram => {
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
        seeds = [ProgramConfig::PREFIX_SEED],
        bump,
        has_one = authority @ LazorKitError::InvalidAuthority
    )]
    pub config: Box<Account<'info, ProgramConfig>>,
}
