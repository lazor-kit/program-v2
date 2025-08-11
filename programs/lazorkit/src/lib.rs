use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod security;
pub mod state;
pub mod utils;

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
        args: CreatwSmartWalletArgs,
    ) -> Result<()> {
        instructions::create_smart_wallet(ctx, args)
    }

    /// Add a program to the whitelist of rule programs
    pub fn add_whitelist_rule_program(ctx: Context<AddWhitelistRuleProgram>) -> Result<()> {
        instructions::add_whitelist_rule_program(ctx)
    }

    pub fn change_rule_direct<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, ChangeRuleDirect<'info>>,
        args: ChangeRuleArgs,
    ) -> Result<()> {
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

    pub fn commit_cpi<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CommitCpi<'info>>,
        args: CommitArgs,
    ) -> Result<()> {
        instructions::commit_cpi(ctx, args)
    }

    pub fn execute_committed(ctx: Context<ExecuteCommitted>, cpi_data: Vec<u8>) -> Result<()> {
        instructions::execute_committed(ctx, cpi_data)
    }
}
