use anchor_lang::prelude::*;

use crate::{
    error::LazorKitError,
    state::{Config, UpdateConfigType},
};

pub fn update_config(
    ctx: Context<UpdateConfig>,
    param: UpdateConfigType,
    value: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    match param {
        UpdateConfigType::CreateWalletFee => {
            // Validate fee is reasonable (max 1 SOL)
            require!(value <= 1_000_000_000, LazorKitError::InvalidFeeAmount);
            config.create_smart_wallet_fee = value;
            msg!("Updated create_smart_wallet_fee to: {}", value);
        }
        UpdateConfigType::FeePayerFee => {
            // Validate fee is reasonable (max 0.1 SOL)
            require!(value <= 100_000_000, LazorKitError::InvalidFeeAmount);
            config.fee_payer_fee = value;
            msg!("Updated fee_payer_fee to: {}", value);
        }
        UpdateConfigType::ReferralFee => {
            // Validate fee is reasonable (max 0.1 SOL)
            require!(value <= 100_000_000, LazorKitError::InvalidFeeAmount);
            config.referral_fee = value;
            msg!("Updated referral_fee to: {}", value);
        }
        UpdateConfigType::LazorkitFee => {
            // Validate fee is reasonable (max 0.1 SOL)
            require!(value <= 100_000_000, LazorKitError::InvalidFeeAmount);
            config.lazorkit_fee = value;
            msg!("Updated lazorkit_fee to: {}", value);
        }
        UpdateConfigType::DefaultPolicyProgram => {
            let new_default_policy_program_info = ctx
                .remaining_accounts
                .first()
                .ok_or(LazorKitError::InvalidRemainingAccounts)?;

            // Check if the new default policy program is executable
            if !new_default_policy_program_info.executable {
                return err!(LazorKitError::ProgramNotExecutable);
            }

            config.default_policy_program = new_default_policy_program_info.key();
            msg!(
                "Updated default_policy_program to: {}",
                new_default_policy_program_info.key()
            );
        }
        UpdateConfigType::Admin => {
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
            msg!("Updated authority to: {}", new_admin_info.key());
        }
        UpdateConfigType::PauseProgram => {
            require!(!config.is_paused, LazorKitError::ProgramPaused);
            config.is_paused = true;
            msg!("Program paused - emergency shutdown activated");
        }
        UpdateConfigType::UnpauseProgram => {
            require!(config.is_paused, LazorKitError::InvalidAccountState);
            config.is_paused = false;
            msg!("Program unpaused - normal operations resumed");
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
