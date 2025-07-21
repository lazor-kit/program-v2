use anchor_lang::prelude::*;

use crate::{
    constants::{PASSKEY_SIZE, SMART_WALLET_SEED},
    error::LazorKitError,
    events::{SmartWalletCreated, FeeCollected},
    security::validation,
    state::{
        Config, SmartWalletAuthenticator, SmartWalletConfig, SmartWalletSeq, WhitelistRulePrograms,
    },
    utils::{execute_cpi, transfer_sol_from_pda, PasskeyExt, PdaSigner},
    ID,
};

pub fn create_smart_wallet(
    ctx: Context<CreateSmartWallet>,
    passkey_pubkey: [u8; PASSKEY_SIZE],
    credential_id: Vec<u8>,
    rule_data: Vec<u8>,
) -> Result<()> {
    // === Input Validation ===
    validation::validate_credential_id(&credential_id)?;
    validation::validate_rule_data(&rule_data)?;
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    
    // Validate passkey format (ensure it's a valid compressed public key)
    require!(
        passkey_pubkey[0] == 0x02 || passkey_pubkey[0] == 0x03,
        LazorKitError::InvalidPasskeyFormat
    );
    
    // === Sequence and Configuration ===
    let wallet_data = &mut ctx.accounts.smart_wallet_config;
    let sequence_account = &mut ctx.accounts.smart_wallet_seq;
    let smart_wallet_authenticator = &mut ctx.accounts.smart_wallet_authenticator;
    
    // Check for potential sequence overflow
    let new_seq = sequence_account.seq
        .checked_add(1)
        .ok_or(LazorKitError::IntegerOverflow)?;
    
    // Validate default rule program
    validation::validate_program_executable(&ctx.accounts.default_rule_program)?;
    
    // === Initialize Smart Wallet Config ===
    wallet_data.set_inner(SmartWalletConfig {
        rule_program: ctx.accounts.config.default_rule_program,
        id: sequence_account.seq,
        last_nonce: 0,
        bump: ctx.bumps.smart_wallet,
    });

    // === Initialize Smart Wallet Authenticator ===
    smart_wallet_authenticator.set_inner(SmartWalletAuthenticator {
        passkey_pubkey,
        smart_wallet: ctx.accounts.smart_wallet.key(),
        credential_id: credential_id.clone(),
        bump: ctx.bumps.smart_wallet_authenticator,
    });
    
    // === Create PDA Signer ===
    let signer = PdaSigner {
        seeds: vec![
            SmartWalletAuthenticator::PREFIX_SEED.to_vec(),
            ctx.accounts.smart_wallet.key().as_ref().to_vec(),
            passkey_pubkey
                .to_hashed_bytes(ctx.accounts.smart_wallet.key())
                .as_ref()
                .to_vec(),
        ],
        bump: ctx.bumps.smart_wallet_authenticator,
    };

    // === Execute Rule Program CPI ===
    execute_cpi(
        &ctx.remaining_accounts,
        &rule_data,
        &ctx.accounts.default_rule_program,
        Some(signer),
    )?;

    // === Update Sequence ===
    sequence_account.seq = new_seq;

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
        
        transfer_sol_from_pda(
            &ctx.accounts.smart_wallet,
            &ctx.accounts.signer,
            fee,
        )?;
    }

    // === Emit Events ===
    msg!("Smart wallet created: {}", ctx.accounts.smart_wallet.key());
    msg!("Authenticator: {}", ctx.accounts.smart_wallet_authenticator.key());
    msg!("Sequence ID: {}", sequence_account.seq.saturating_sub(1));
    
    // Emit wallet creation event
    SmartWalletCreated::emit_event(
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.smart_wallet_authenticator.key(),
        sequence_account.seq.saturating_sub(1),
        ctx.accounts.config.default_rule_program,
        passkey_pubkey,
    )?;
    
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

    Ok(())
}

#[derive(Accounts)]
#[instruction(passkey_pubkey: [u8; PASSKEY_SIZE], credential_id: Vec<u8>, rule_data: Vec<u8>)]
pub struct CreateSmartWallet<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// Smart wallet sequence tracker
    #[account(
        mut,
        seeds = [SmartWalletSeq::PREFIX_SEED],
        bump,
        constraint = smart_wallet_seq.seq < u64::MAX @ LazorKitError::MaxWalletLimitReached
    )]
    pub smart_wallet_seq: Account<'info, SmartWalletSeq>,

    /// Whitelist of allowed rule programs
    #[account(
        seeds = [WhitelistRulePrograms::PREFIX_SEED],
        bump,
        owner = ID,
        constraint = whitelist_rule_programs.list.contains(&default_rule_program.key()) @ LazorKitError::RuleProgramNotWhitelisted
    )]
    pub whitelist_rule_programs: Account<'info, WhitelistRulePrograms>,

    /// The smart wallet PDA being created
    #[account(
        init,
        payer = signer,
        space = 0,
        seeds = [SMART_WALLET_SEED, smart_wallet_seq.seq.to_le_bytes().as_ref()],
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
            passkey_pubkey.to_hashed_bytes(smart_wallet.key()).as_ref()
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
