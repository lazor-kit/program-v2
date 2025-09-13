use anchor_lang::{prelude::*, solana_program::system_instruction};

use crate::{
    constants::SMART_WALLET_SEED,
    error::LazorKitError,
    events::{FeeCollected, SmartWalletCreated},
    instructions::CreateSmartWalletArgs,
    security::validation,
    state::{Config, PolicyProgramRegistry, SmartWallet, WalletDevice},
    utils::{execute_cpi, PasskeyExt, PdaSigner},
    ID,
};

pub fn create_smart_wallet(
    ctx: Context<CreateSmartWallet>,
    args: CreateSmartWalletArgs,
) -> Result<()> {
    // Program must not be paused
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    // === Input Validation ===
    validation::validate_credential_id(&args.credential_id)?;
    validation::validate_policy_data(&args.policy_data)?;
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;

    // Validate passkey format (ensure it's a valid compressed public key)
    require!(
        args.passkey_pubkey[0] == 0x02 || args.passkey_pubkey[0] == 0x03,
        LazorKitError::InvalidPasskeyFormat
    );

    // Validate wallet ID is not zero (reserved) and not too large
    require!(
        args.wallet_id != 0 && args.wallet_id < u64::MAX,
        LazorKitError::InvalidSequenceNumber
    );

    // === Configuration ===
    let wallet_data = &mut ctx.accounts.smart_wallet_data;
    let wallet_device = &mut ctx.accounts.wallet_device;

    // Validate default policy program
    validation::validate_program_executable(&ctx.accounts.default_policy_program)?;

    // === Initialize Smart Wallet ===
    wallet_data.set_inner(SmartWallet {
        policy_program: ctx.accounts.config.default_policy_program,
        id: args.wallet_id,
        last_nonce: 0,
        bump: ctx.bumps.smart_wallet,
        referral: args.referral.unwrap_or(ctx.accounts.payer.key()),
    });

    // === Initialize Wallet Device ===
    wallet_device.set_inner(WalletDevice {
        passkey_pubkey: args.passkey_pubkey,
        smart_wallet: ctx.accounts.smart_wallet.key(),
        credential_id: args.credential_id.clone(),
        bump: ctx.bumps.wallet_device,
    });

    // === Create PDA Signer ===
    let signer = PdaSigner {
        seeds: vec![
            WalletDevice::PREFIX_SEED.to_vec(),
            ctx.accounts.smart_wallet.key().as_ref().to_vec(),
            args.passkey_pubkey
                .to_hashed_bytes(ctx.accounts.smart_wallet.key())
                .as_ref()
                .to_vec(),
        ],
        bump: ctx.bumps.wallet_device,
        owner: ctx.accounts.system_program.key(),
    };

    // === Execute Policy Program CPI ===
    execute_cpi(
        &ctx.remaining_accounts,
        &args.policy_data,
        &ctx.accounts.default_policy_program,
        signer.clone(),
        &[ctx.accounts.payer.key()],
    )?;

    if !args.is_pay_for_user {
        // === Collect Creation Fee ===
        let fee = ctx.accounts.config.create_smart_wallet_fee;
        if fee > 0 {
            // Ensure the smart wallet has sufficient balance after fee deduction
            let smart_wallet_balance = ctx.accounts.smart_wallet.lamports();
            let rent = Rent::get()?.minimum_balance(0);

            require!(
                smart_wallet_balance >= fee + rent,
                LazorKitError::InsufficientBalanceForFee
            );

            let transfer = system_instruction::transfer(
                &ctx.accounts.smart_wallet.key(),
                &ctx.accounts.payer.key(),
                fee,
            );

            execute_cpi(
                &[
                    ctx.accounts.smart_wallet.to_account_info(),
                    ctx.accounts.payer.to_account_info(),
                ],
                &transfer.data,
                &ctx.accounts.system_program,
                signer.clone(),
                &[],
            )?;

            emit!(FeeCollected {
                smart_wallet: ctx.accounts.smart_wallet.key(),
                fee_type: "CREATE_WALLET".to_string(),
                amount: fee,
                recipient: ctx.accounts.payer.key(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
    }

    // === Emit Events ===
    msg!("Smart wallet created: {}", ctx.accounts.smart_wallet.key());
    msg!("Device: {}", ctx.accounts.wallet_device.key());
    msg!("Wallet ID: {}", args.wallet_id);

    // Emit wallet creation event
    SmartWalletCreated::emit_event(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.key(),
        args.wallet_id,
        ctx.accounts.config.default_policy_program,
        args.passkey_pubkey,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CreateSmartWalletArgs)]
pub struct CreateSmartWallet<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Policy program registry
    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID,
        constraint = policy_program_registry.programs.contains(&default_policy_program.key()) @ LazorKitError::PolicyProgramNotRegistered
    )]
    pub policy_program_registry: Account<'info, PolicyProgramRegistry>,

    /// The smart wallet PDA being created with random ID
    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, args.wallet_id.to_le_bytes().as_ref()],
        bump,
    )]
    /// CHECK: This account is only used for its public key and seeds.
    pub smart_wallet: SystemAccount<'info>,

    /// Smart wallet data
    #[account(
        init,
        payer = payer,
        space = 8 + SmartWallet::INIT_SPACE,
        seeds = [SmartWallet::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump
    )]
    pub smart_wallet_data: Box<Account<'info, SmartWallet>>,

    /// Wallet device for the passkey
    #[account(
        init,
        payer = payer,
        space = 8 + WalletDevice::INIT_SPACE,
        seeds = [
            WalletDevice::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.passkey_pubkey.to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    /// Program configuration
    #[account(
        seeds = [Config::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub config: Box<Account<'info, Config>>,

    /// Default policy program for the smart wallet
    #[account(
        address = config.default_policy_program,
        executable,
        constraint = default_policy_program.executable @ LazorKitError::ProgramNotExecutable
    )]
    /// CHECK: Validated to be executable and in registry
    pub default_policy_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
