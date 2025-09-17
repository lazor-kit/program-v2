use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

use crate::{
    constants::SMART_WALLET_SEED,
    error::LazorKitError,
    instructions::CreateSmartWalletArgs,
    security::validation,
    state::{PolicyProgramRegistry, Config, SmartWalletData, WalletDevice},
    utils::{execute_cpi, PasskeyExt, PdaSigner},
    ID,
};

/// Create a new smart wallet with WebAuthn passkey authentication
/// 
/// This function initializes a new smart wallet with the following steps:
/// 1. Validates input parameters and program state
/// 2. Creates the smart wallet data account
/// 3. Creates the associated wallet device (passkey) account
/// 4. Transfers initial SOL to the smart wallet
/// 5. Executes the policy program initialization
/// 
/// # Arguments
/// * `ctx` - The instruction context containing all required accounts
/// * `args` - The creation arguments including passkey, policy data, and wallet ID
/// 
/// # Returns
/// * `Result<()>` - Success if the wallet is created successfully
pub fn create_smart_wallet(
    ctx: Context<CreateSmartWallet>,
    args: CreateSmartWalletArgs,
) -> Result<()> {
    // Step 1: Validate global program state and input parameters
    // Ensure the program is not paused before processing wallet creation
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    
    // Validate all input parameters for security and correctness
    validation::validate_credential_id(&args.credential_id)?;
    validation::validate_policy_data(&args.policy_data)?;
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;

    // Validate passkey format - must be a valid compressed public key (starts with 0x02 or 0x03)
    require!(
        args.passkey_public_key[0] == 0x02 || args.passkey_public_key[0] == 0x03,
        LazorKitError::InvalidPasskeyFormat
    );

    // Validate wallet ID is not zero (reserved) and within valid range
    require!(
        args.wallet_id != 0 && args.wallet_id < u64::MAX,
        LazorKitError::InvalidSequenceNumber
    );

    // Step 2: Prepare account references and validate policy program
    let wallet_data = &mut ctx.accounts.smart_wallet_data;
    let wallet_device = &mut ctx.accounts.wallet_device;

    // Ensure the default policy program is executable (not a data account)
    validation::validate_program_executable(&ctx.accounts.default_policy_program)?;

    // Step 3: Initialize the smart wallet data account
    // This stores the core wallet state including policy program, nonce, and referral info
    wallet_data.set_inner(SmartWalletData {
        policy_program_id: ctx.accounts.config.default_policy_program_id,
        wallet_id: args.wallet_id,
        last_nonce: 0, // Start with nonce 0 for replay attack prevention
        bump: ctx.bumps.smart_wallet,
        referral_address: args.referral_address.unwrap_or(ctx.accounts.payer.key()),
    });

    // Step 4: Initialize the wallet device (passkey) account
    // This stores the WebAuthn passkey data for transaction authentication
    wallet_device.set_inner(WalletDevice {
        passkey_public_key: args.passkey_public_key,
        smart_wallet_address: ctx.accounts.smart_wallet.key(),
        credential_id: args.credential_id.clone(),
        bump: ctx.bumps.wallet_device,
    });

    // Step 5: Transfer initial SOL to the smart wallet
    // This provides the wallet with initial funding for transactions and rent
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

    // Step 6: Create PDA signer for policy program initialization
    // This allows the smart wallet to sign calls to the policy program
    let wallet_signer = PdaSigner {
        seeds: vec![
            SMART_WALLET_SEED.to_vec(),
            args.wallet_id.to_le_bytes().to_vec(),
        ],
        bump: ctx.bumps.smart_wallet,
    };

    // Step 7: Initialize the policy program for this wallet
    // This sets up the policy program with any required initial state
    execute_cpi(
        &ctx.remaining_accounts,
        &args.policy_data,
        &ctx.accounts.default_policy_program,
        wallet_signer.clone(),
    )?;

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
        constraint = policy_program_registry.registered_programs.contains(&default_policy_program.key()) @ LazorKitError::PolicyProgramNotRegistered
    )]
    pub policy_program_registry: Account<'info, PolicyProgramRegistry>,

    /// The smart wallet address PDA being created with the provided wallet ID
    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, args.wallet_id.to_le_bytes().as_ref()],
        bump,
    )]
    /// CHECK: This account is only used for its public key and seeds.
    pub smart_wallet: SystemAccount<'info>,

    /// Smart wallet data account that stores wallet state and configuration
    #[account(
        init,
        payer = payer,
        space = 8 + SmartWalletData::INIT_SPACE,
        seeds = [SmartWalletData::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump
    )]
    pub smart_wallet_data: Box<Account<'info, SmartWalletData>>,

    /// Wallet device account that stores the passkey authentication data
    #[account(
        init,
        payer = payer,
        space = 8 + WalletDevice::INIT_SPACE,
        seeds = [
            WalletDevice::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.passkey_public_key.to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    /// Program configuration account containing global settings
    #[account(
        seeds = [Config::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub config: Box<Account<'info, Config>>,

    /// Default policy program that will govern this smart wallet's transactions
    #[account(
        address = config.default_policy_program_id,
        executable,
        constraint = default_policy_program.executable @ LazorKitError::ProgramNotExecutable
    )]
    /// CHECK: Validated to be executable and in registry
    pub default_policy_program: UncheckedAccount<'info>,

    /// System program for account creation and SOL transfers
    pub system_program: Program<'info, System>,
}
