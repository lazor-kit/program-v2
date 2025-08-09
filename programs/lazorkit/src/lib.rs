use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod security;
pub mod state;
pub mod utils;

use constants::PASSKEY_SIZE;
use instructions::*;
use state::*;

declare_id!("J6Big9w1VNeRZgDWH5qmNz2Nd6XFq5QeZbqC8caqSE5W");

/// The Lazor Kit program provides smart wallet functionality with passkey authentication
#[program]
pub mod lazorkit {
    use super::*;

    /// Initialize the program by creating the sequence tracker
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize(ctx)
    }

    /// Update the program configuration
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        param: UpdateConfigType,
        value: u64,
    ) -> Result<()> {
        instructions::update_config(ctx, param, value)
    }

    /// Create a new smart wallet with passkey authentication
    pub fn create_smart_wallet(
        ctx: Context<CreateSmartWallet>,
        passkey_pubkey: [u8; PASSKEY_SIZE],
        credential_id: Vec<u8>,
        rule_data: Vec<u8>,
        wallet_id: u64,
        is_pay_for_user: bool,
    ) -> Result<()> {
        instructions::create_smart_wallet(
            ctx,
            passkey_pubkey,
            credential_id,
            rule_data,
            wallet_id,
            is_pay_for_user,
        )
    }

    /// Add a program to the whitelist of rule programs
    pub fn add_whitelist_rule_program(ctx: Context<AddWhitelistRuleProgram>) -> Result<()> {
        instructions::add_whitelist_rule_program(ctx)
    }

    pub fn change_rule_direct(ctx: Context<ChangeRuleDirect>, args: ChangeRuleArgs) -> Result<()> {
        instructions::change_rule_direct(ctx, args)
    }

    pub fn call_rule_direct<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CallRuleDirect<'info>>,
        args: CallRuleArgs,
    ) -> Result<()> {
        instructions::call_rule_direct(ctx, args)
    }

    pub fn execute_txn_direct<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, ExecuteTxn<'info>>,
        args: ExecuteTxnArgs,
    ) -> Result<()> {
        instructions::execute_txn_direct(ctx, args)
    }

    /// Commit a CPI after verifying auth and rule. Stores data and constraints.
    pub fn commit_cpi<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CommitCpi<'info>>,
        args: CommitArgs,
    ) -> Result<()> {
        instructions::commit_cpi(ctx, args)
    }

    /// Execute a previously committed CPI (no passkey verification here).
    pub fn execute_committed(
        ctx: Context<ExecuteCommitted>,
        args: ExecuteCommittedArgs,
    ) -> Result<()> {
        instructions::execute_committed(ctx, args)
    }
}
