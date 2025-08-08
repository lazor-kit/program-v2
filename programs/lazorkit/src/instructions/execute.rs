//! Unified smart-wallet instruction dispatcher.
//!
//! External callers only need to invoke **one** instruction (`execute`) and
//! specify the desired `Action`.  Internally we forward to specialised
//! handler functions located in `handlers/`.

// -----------------------------------------------------------------------------
//  Imports
// -----------------------------------------------------------------------------
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::ID as IX_ID;

use crate::security::validation;
use crate::state::{Config, SmartWalletAuthenticator, SmartWalletConfig, WhitelistRulePrograms};
use crate::utils::{verify_authorization, PasskeyExt};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

use super::handlers::{call_rule, execute_tx, change_rule};

/// Supported wallet actions
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    ExecuteTx,
    ChangeRuleProgram,
    CallRuleProgram,
}

/// Single args struct shared by all actions
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ExecuteArgs {
    pub passkey_pubkey: [u8; 33],
    pub signature: Vec<u8>,
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub action: Action,
    /// optional new authenticator passkey (only for `CallRuleProgram`)
    pub create_new_authenticator: Option<[u8; 33]>,
}

impl ExecuteArgs {
    /// Validate execute arguments
    pub fn validate(&self) -> Result<()> {
        // Validate passkey format
        require!(
            self.passkey_pubkey[0] == 0x02 || self.passkey_pubkey[0] == 0x03,
            LazorKitError::InvalidPasskeyFormat
        );
        
        // Validate signature length (Secp256r1 signature should be 64 bytes)
        require!(
            self.signature.len() == 64,
            LazorKitError::InvalidSignature
        );
        
        // Validate client data and authenticator data are not empty
        require!(
            !self.client_data_json_raw.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        require!(
            !self.authenticator_data_raw.is_empty(),
            LazorKitError::InvalidInstructionData
        );
        
        // Validate verify instruction index
        require!(
            self.verify_instruction_index < 255,
            LazorKitError::InvalidInstructionData
        );
        
        // Validate new authenticator if provided
        if let Some(new_auth) = self.create_new_authenticator {
            require!(
                new_auth[0] == 0x02 || new_auth[0] == 0x03,
                LazorKitError::InvalidPasskeyFormat
            );
            
            // Only CallRuleProgram action can create new authenticator
            require!(
                self.action == Action::CallRuleProgram,
                LazorKitError::InvalidActionType
            );
        }
        
        Ok(())
    }
}

/// Single entry-point for all smart-wallet interactions
pub fn execute<'c: 'info, 'info>(
    mut ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
    args: ExecuteArgs,
) -> Result<()> {
    // ------------------------------------------------------------------
    // 1. Input Validation
    // ------------------------------------------------------------------
    args.validate()?;
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    
    // Check if program is paused (emergency shutdown)
    require!(
        !ctx.accounts.config.is_paused,
        LazorKitError::ProgramPaused
    );
    
    // Validate smart wallet state
    require!(
        ctx.accounts.smart_wallet_config.id < u64::MAX,
        LazorKitError::InvalidWalletConfiguration
    );
    
    // ------------------------------------------------------------------
    // 2. Authorization (shared)
    // ------------------------------------------------------------------
    let msg = verify_authorization(
        &ctx.accounts.ix_sysvar,
        &ctx.accounts.smart_wallet_authenticator,
        ctx.accounts.smart_wallet.key(),
        args.passkey_pubkey,
        args.signature.clone(),
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        ctx.accounts.smart_wallet_config.last_nonce,
    )?;

    // Additional validation on the message
    if let Some(ref rule_data) = msg.rule_data {
        validation::validate_rule_data(rule_data)?;
    }
    validation::validate_cpi_data(&msg.cpi_data)?;
    
    // Validate split index
    let total_accounts = ctx.remaining_accounts.len();
    require!(
        (msg.split_index as usize) <= total_accounts,
        LazorKitError::InvalidSplitIndex
    );

    // ------------------------------------------------------------------
    // 3. Dispatch to specialised handler
    // ------------------------------------------------------------------
    msg!("Executing action: {:?}", args.action);
    msg!("Smart wallet: {}", ctx.accounts.smart_wallet.key());
    msg!("Nonce: {}", ctx.accounts.smart_wallet_config.last_nonce);
    
    match args.action {
        Action::ExecuteTx => {
            execute_tx::handle(&mut ctx, &args, &msg)?;
        }
        Action::ChangeRuleProgram => {
            change_rule::handle(&mut ctx, &args, &msg)?;
        }
        Action::CallRuleProgram => {
            call_rule::handle(&mut ctx, &args, &msg)?;
        }
    }

    // ------------------------------------------------------------------
    // 4. Post-execution updates
    // ------------------------------------------------------------------
    
    // Increment nonce with overflow protection
    ctx.accounts.smart_wallet_config.last_nonce = ctx
        .accounts
        .smart_wallet_config
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;
    
    // Collect execution fee if configured
    let fee = ctx.accounts.config.execute_fee;
    if fee > 0 {
        // Check smart wallet has sufficient balance
        let smart_wallet_balance = ctx.accounts.smart_wallet.lamports();
        let rent = Rent::get()?.minimum_balance(0);
        
        require!(
            smart_wallet_balance >= fee + rent,
            LazorKitError::InsufficientBalanceForFee
        );
        
        crate::utils::transfer_sol_from_pda(
            &ctx.accounts.smart_wallet,
            &ctx.accounts.payer,
            fee,
        )?;
    }
    
    // Emit execution event
    msg!("Action executed successfully");
    msg!("New nonce: {}", ctx.accounts.smart_wallet_config.last_nonce);

    Ok(())
}

// -----------------------------------------------------------------------------
//  Anchor account context – superset of all action requirements
// -----------------------------------------------------------------------------
#[derive(Accounts)]
#[instruction(args: ExecuteArgs)]
pub struct Execute<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [Config::PREFIX_SEED], 
        bump, 
        owner = ID
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
        owner = ID,
    )]
    /// CHECK: Validated through seeds and bump
    pub smart_wallet: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
        constraint = smart_wallet_config.rule_program != Pubkey::default() @ LazorKitError::InvalidWalletConfiguration
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    #[account(
        seeds = [
            SmartWalletAuthenticator::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.passkey_pubkey.to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump = smart_wallet_authenticator.bump,
        owner = ID,
        constraint = smart_wallet_authenticator.smart_wallet == smart_wallet.key() @ LazorKitError::SmartWalletMismatch,
        constraint = smart_wallet_authenticator.passkey_pubkey == args.passkey_pubkey @ LazorKitError::PasskeyMismatch
    )]
    pub smart_wallet_authenticator: Box<Account<'info, SmartWalletAuthenticator>>,

    #[account(
        seeds = [WhitelistRulePrograms::PREFIX_SEED], 
        bump, 
        owner = ID
    )]
    pub whitelist_rule_programs: Box<Account<'info, WhitelistRulePrograms>>,

    /// CHECK: Validated in handlers based on action type
    pub authenticator_program: UncheckedAccount<'info>,

    #[account(
        address = IX_ID,
        constraint = ix_sysvar.key() == IX_ID @ LazorKitError::InvalidAccountData
    )]
    /// CHECK: instruction sysvar validated by address
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: Validated in handlers based on action type
    pub cpi_program: UncheckedAccount<'info>,

    /// The new authenticator is an optional account that is only initialized
    /// by the `CallRuleProgram` action. It is passed as an UncheckedAccount
    /// and created via CPI if needed.
    pub new_smart_wallet_authenticator: Option<UncheckedAccount<'info>>,

    // No blob in this path
}
