use std::vec;

use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

use crate::{
    constants::SMART_WALLET_SEED,
    error::LazorKitError,
    instructions::CreateSmartWalletArgs,
    security::validation,
    state::{Config, PolicyProgramRegistry, WalletDevice, WalletState},
    utils::{create_wallet_device_hash, execute_cpi, get_policy_signer},
    ID,
};

pub fn create_smart_wallet(
    ctx: Context<CreateSmartWallet>,
    args: CreateSmartWalletArgs,
) -> Result<()> {
    // Ensure the program is not paused before processing wallet creation
    require!(
        !ctx.accounts.lazorkit_config.is_paused,
        LazorKitError::ProgramPaused
    );

    // Validate all input parameters for security and correctness
    validation::validate_passkey_format(&args.passkey_public_key)?;
    validation::validate_policy_data(&args.init_policy_data)?;
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_no_reentrancy(&ctx.remaining_accounts)?;
    validation::validate_wallet_id(args.wallet_id)?;

    // Get the policy signer for the wallet device
    let policy_signer = get_policy_signer(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.key(),
        args.credential_hash,
    )?;

    // Execute the policy program initialization
    let policy_data = execute_cpi(
        &ctx.remaining_accounts,
        &args.init_policy_data.clone(),
        &ctx.accounts.policy_program,
        policy_signer.clone(),
    )?;

    // Initialize the wallet state
    let wallet_state = &mut ctx.accounts.wallet_state;
    wallet_state.set_inner(WalletState {
        bump: ctx.bumps.smart_wallet,
        wallet_id: args.wallet_id,
        last_nonce: 0u64,
        referral: args.referral_address.unwrap_or(ctx.accounts.payer.key()),
        policy_program: ctx.accounts.policy_program.key(),
        policy_data,
    });

    // Initialize the wallet device
    let wallet_device = &mut ctx.accounts.wallet_device;
    wallet_device.set_inner(WalletDevice {
        bump: ctx.bumps.wallet_device,
        passkey_pubkey: args.passkey_public_key,
        credential_hash: args.credential_hash,
        smart_wallet: ctx.accounts.smart_wallet.key(),
    });

    if args.amount > 0 {
        transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.smart_wallet.to_account_info(),
                },
            ),
            args.amount,
        )?;
    }

    // Check that smart-wallet balance >= empty rent exempt balance
    require!(
        ctx.accounts.smart_wallet.lamports() >= crate::constants::EMPTY_PDA_RENT_EXEMPT_BALANCE,
        LazorKitError::InsufficientBalanceForFee
    );

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CreateSmartWalletArgs)]
pub struct CreateSmartWallet<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID,
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, args.wallet_id.to_le_bytes().as_ref()],
        bump,
    )]
    /// CHECK: PDA verified by seeds
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        init,
        payer = payer,
        space = 8 + WalletState::INIT_SPACE,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        init,
        payer = payer,
        space = 8 + WalletDevice::INIT_SPACE,
        seeds = [WalletDevice::PREFIX_SEED, &create_wallet_device_hash(smart_wallet.key(), args.credential_hash)],
        bump
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    #[account(
        seeds = [Config::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub lazorkit_config: Box<Account<'info, Config>>,

    #[account(
        executable,
        constraint = policy_program.executable @ LazorKitError::ProgramNotExecutable,
        constraint = policy_program_registry.registered_programs.contains(&policy_program.key()) @ LazorKitError::PolicyProgramNotRegistered
    )]
    /// CHECK: Validated to be executable and in registry
    pub policy_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
