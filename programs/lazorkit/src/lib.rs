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
        args: CreateSmartWalletArgs,
    ) -> Result<()> {
        instructions::create_smart_wallet(ctx, args)
    }

    /// Add a program to the policy program registry
    pub fn register_policy_program(ctx: Context<RegisterPolicyProgram>) -> Result<()> {
        instructions::register_policy_program(ctx)
    }

    pub fn update_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, UpdatePolicy<'info>>,
        args: UpdatePolicyArgs,
    ) -> Result<()> {
        instructions::update_policy(ctx, args)
    }

    pub fn invoke_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, InvokePolicy<'info>>,
        args: InvokePolicyArgs,
    ) -> Result<()> {
        instructions::invoke_policy(ctx, args)
    }

    pub fn execute_transaction<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, ExecuteTransaction<'info>>,
        args: ExecuteTransactionArgs,
    ) -> Result<()> {
        instructions::execute_transaction(ctx, args)
    }

    pub fn create_transaction_session<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CreateTransactionSession<'info>>,
        args: CreateSessionArgs,
    ) -> Result<()> {
        instructions::create_transaction_session(ctx, args)
    }

    pub fn execute_session_transaction(
        ctx: Context<ExecuteSessionTransaction>,
        vec_cpi_data: Vec<Vec<u8>>,
        split_index: Vec<u8>,
    ) -> Result<()> {
        instructions::execute_session_transaction(ctx, vec_cpi_data, split_index)
    }

    /// Initialize a new vault
    pub fn initialize_vault(ctx: Context<InitializeVault>, index: u8) -> Result<()> {
        instructions::initialize_vault(ctx, index)
    }

    /// Withdraw SOL from vault
    pub fn withdraw_vault(ctx: Context<WithdrawVault>, amount: u64) -> Result<()> {
        instructions::withdraw_vault(ctx, amount)
    }
}
