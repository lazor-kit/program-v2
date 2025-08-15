use anchor_lang::prelude::*;

use crate::{
    constants::SMART_WALLET_SEED,
    error::LazorKitError,
    events::{FeeCollected, SmartWalletCreated},
    instructions::CreatwSmartWalletArgs,
    security::validation,
    state::{Config, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms},
    utils::{execute_cpi, transfer_sol_from_pda, PasskeyExt, PdaSigner},
    ID,
};

pub fn create_smart_wallet(
    ctx: Context<CreateSmartWallet>,
    args: CreatwSmartWalletArgs,
) -> Result<()> {
    // Program must not be paused
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    // === Input Validation ===
    validation::validate_credential_id(&args.credential_id)?;
    validation::validate_rule_data(&args.rule_data)?;
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;

    // Validate passkey format (ensure it's a valid compressed public key)
    require!(
        args.passkey_pubkey[0] == 0x02 || args.passkey_pubkey[0] == 0x03,
        LazorKitError::InvalidPasskeyFormat
    );

    // Validate wallet ID is not zero (reserved)
    require!(args.wallet_id != 0, LazorKitError::InvalidSequenceNumber);

    // Additional validation: ensure wallet ID is within reasonable bounds
    require!(
        args.wallet_id < u64::MAX,
        LazorKitError::InvalidSequenceNumber
    );

    // === Configuration ===
    let wallet_data = &mut ctx.accounts.smart_wallet_config;
    let smart_wallet_authenticator = &mut ctx.accounts.smart_wallet_authenticator;

    // Validate default rule program
    validation::validate_program_executable(&ctx.accounts.default_rule_program)?;

    // === Initialize Smart Wallet Config ===
    wallet_data.set_inner(SmartWalletConfig {
        rule_program: ctx.accounts.config.default_rule_program,
        id: args.wallet_id,
        last_nonce: 0,
        bump: ctx.bumps.smart_wallet,
    });

    // === Initialize Smart Wallet Authenticator ===
    smart_wallet_authenticator.set_inner(SmartWalletAuthenticator {
        passkey_pubkey: args.passkey_pubkey,
        smart_wallet: ctx.accounts.smart_wallet.key(),
        credential_id: args.credential_id.clone(),
        bump: ctx.bumps.smart_wallet_authenticator,
    });

    // === Create PDA Signer ===
    let signer = PdaSigner {
        seeds: vec![
            SmartWalletAuthenticator::PREFIX_SEED.to_vec(),
            ctx.accounts.smart_wallet.key().as_ref().to_vec(),
            args.passkey_pubkey
                .to_hashed_bytes(ctx.accounts.smart_wallet.key())
                .as_ref()
                .to_vec(),
        ],
        bump: ctx.bumps.smart_wallet_authenticator,
    };

    // === Execute Rule Program CPI ===
    execute_cpi(
        &ctx.remaining_accounts,
        &args.rule_data,
        &ctx.accounts.default_rule_program,
        Some(signer),
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

            transfer_sol_from_pda(&ctx.accounts.smart_wallet, &ctx.accounts.signer, fee)?;
        }

        // Emit fee collection event if fee was charged
        if fee > 0 {
            emit!(FeeCollected {
                smart_wallet: ctx.accounts.smart_wallet.key(),
                fee_type: "CREATE_WALLET".to_string(),
                amount: fee,
                recipient: ctx.accounts.signer.key(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
    }

    // === Emit Events ===
    msg!("Smart wallet created: {}", ctx.accounts.smart_wallet.key());
    msg!(
        "Authenticator: {}",
        ctx.accounts.smart_wallet_authenticator.key()
    );
    msg!("Wallet ID: {}", args.wallet_id);

    // Emit wallet creation event
    SmartWalletCreated::emit_event(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.key(),
        args.wallet_id,
        ctx.accounts.config.default_rule_program,
        args.passkey_pubkey,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CreatwSmartWalletArgs)]
pub struct CreateSmartWallet<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// Whitelist of allowed rule programs
    #[account(
        seeds = [WhitelistRulePrograms::PREFIX_SEED],
        bump,
        owner = ID,
        constraint = whitelist_rule_programs.list.contains(&default_rule_program.key()) @ LazorKitError::RuleProgramNotWhitelisted
    )]
    pub whitelist_rule_programs: Account<'info, WhitelistRulePrograms>,

    /// The smart wallet PDA being created with random ID
    #[account(
        init,
        payer = signer,
        space = 0,
        seeds = [SMART_WALLET_SEED, args.wallet_id.to_le_bytes().as_ref()],
        bump
    )]
    /// CHECK: This account is only used for its public key and seeds.
    pub smart_wallet: UncheckedAccount<'info>,

    /// Smart wallet configuration data
    #[account(
        init,
        payer = signer,
        space = 8 + SmartWalletConfig::INIT_SPACE,
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    /// Smart wallet authenticator for the passkey
    #[account(
        init,
        payer = signer,
        space = 8 + SmartWalletAuthenticator::INIT_SPACE,
        seeds = [
            SmartWalletAuthenticator::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.passkey_pubkey.to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump
    )]
    pub smart_wallet_authenticator: Box<Account<'info, SmartWalletAuthenticator>>,

    /// Program configuration
    #[account(
        seeds = [Config::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub config: Box<Account<'info, Config>>,

    /// Default rule program for the smart wallet
    #[account(
        address = config.default_rule_program,
        executable,
        constraint = default_rule_program.executable @ LazorKitError::ProgramNotExecutable
    )]
    /// CHECK: Validated to be executable and in whitelist
    pub default_rule_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
