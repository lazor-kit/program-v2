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
    state::{Config, DeviceSlot, PolicyProgramRegistry, WalletState},
    utils::{execute_cpi, get_policy_signer},
    ID,
};

pub fn create_smart_wallet(
    ctx: Context<CreateSmartWallet>,
    args: CreateSmartWalletArgs,
) -> Result<()> {
    // Step 1: Validate global program state and input parameters
    // Ensure the program is not paused before processing wallet creation
    require!(
        !ctx.accounts.lazorkit_config.is_paused,
        LazorKitError::ProgramPaused
    );

    // Validate all input parameters for security and correctness
    validation::validate_policy_data(&args.init_policy_data)?;
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_no_reentrancy(&ctx.remaining_accounts)?;

    // Validate passkey format - must be a valid compressed public key
    require!(
        args.passkey_public_key[0] == crate::constants::SECP256R1_COMPRESSED_PUBKEY_PREFIX_EVEN
            || args.passkey_public_key[0]
                == crate::constants::SECP256R1_COMPRESSED_PUBKEY_PREFIX_ODD,
        LazorKitError::InvalidPasskeyFormat
    );

    // Validate wallet ID is not zero (reserved) and within valid range
    require!(
        args.wallet_id != 0 && args.wallet_id < u64::MAX,
        LazorKitError::InvalidSequenceNumber
    );

    let cpi_signer = get_policy_signer(
        ctx.accounts.policy_signer.key(),
        args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
    )?;

    let policy_data = execute_cpi(
        &ctx.remaining_accounts,
        &args.init_policy_data.clone(),
        &ctx.accounts.policy_program,
        cpi_signer.clone(),
    )?;

    let wallet_state = &mut ctx.accounts.wallet_state;
    wallet_state.set_inner(WalletState {
        bump: ctx.bumps.smart_wallet,
        wallet_id: args.wallet_id,
        last_nonce: 0,
        referral: args.referral_address.unwrap_or(ctx.accounts.payer.key()),
        policy_program: ctx.accounts.policy_program.key(),
        policy_data_len: policy_data.len() as u16,
        policy_data,
        device_count: 1,
        devices: vec![DeviceSlot {
            passkey_pubkey: args.passkey_public_key,
            credential_hash: args.credential_hash,
        }],
    });

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

    // check that smart-wallet balance >= empty rent exempt balance
    require!(
        ctx.accounts.smart_wallet.lamports() >= crate::constants::EMPTY_PDA_RENT_EXEMPT_BALANCE,
        LazorKitError::InsufficientBalanceForFee
    );

    Ok(())
}

/// Account structure for creating a new smart wallet
///
/// This struct defines all the accounts required to create a new smart wallet,
/// including validation constraints to ensure proper initialization and security.
#[derive(Accounts)]
#[instruction(args: CreateSmartWalletArgs)]
pub struct CreateSmartWallet<'info> {
    /// The account that pays for the wallet creation and initial SOL transfer
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Policy program registry that validates the default policy program
    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID,
    )]
    pub policy_program_registry: Account<'info, PolicyProgramRegistry>,

    /// The smart wallet address PDA being created with the provided wallet ID
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

    /// CHECK: PDA verified by seeds
    pub policy_signer: UncheckedAccount<'info>,

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

    /// System program for account creation and SOL transfers
    pub system_program: Program<'info, System>,
}
