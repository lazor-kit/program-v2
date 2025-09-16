use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
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
    pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
        instructions::initialize_program(ctx)
    }

    /// Update the program configuration
    pub fn update_program_config(
        ctx: Context<UpdateProgramConfig>,
        param: ConfigUpdateType,
        value: u64,
    ) -> Result<()> {
        instructions::update_program_config(ctx, param, value)
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

    pub fn update_wallet_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, UpdateWalletPolicy<'info>>,
        args: UpdateWalletPolicyArgs,
    ) -> Result<()> {
        instructions::update_wallet_policy(ctx, args)
    }

    pub fn invoke_wallet_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, InvokeWalletPolicy<'info>>,
        args: InvokeWalletPolicyArgs,
    ) -> Result<()> {
        instructions::invoke_wallet_policy(ctx, args)
    }

    pub fn execute_direct_transaction<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, ExecuteDirectTransaction<'info>>,
        args: ExecuteDirectTransactionArgs,
    ) -> Result<()> {
        instructions::execute_direct_transaction(ctx, args)
    }

    pub fn create_deferred_execution<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CreateDeferredExecution<'info>>,
        args: CreateDeferredExecutionArgs,
    ) -> Result<()> {
        instructions::create_deferred_execution(ctx, args)
    }

    pub fn execute_deferred_transaction(
        ctx: Context<ExecuteDeferredTransaction>,
        instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
        split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
        vault_index: u8,
    ) -> Result<()> {
        instructions::execute_deferred_transaction(
            ctx,
            instruction_data_list,
            split_index,
            vault_index,
        )
    }

    /// Withdraw SOL from vault
    pub fn manage_vault(
        ctx: Context<ManageVault>,
        action: u8,
        amount: u64,
        index: u8,
    ) -> Result<()> {
        instructions::manage_vault(ctx, action, amount, index)
    }

    /// Authorize ephemeral execution for temporary program access
    pub fn authorize_ephemeral_execution<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, AuthorizeEphemeralExecution<'info>>,
        args: AuthorizeEphemeralExecutionArgs,
    ) -> Result<()> {
        instructions::authorize_ephemeral_execution(ctx, args)
    }

    /// Execute transactions using ephemeral authorization
    pub fn execute_ephemeral_authorization(
        ctx: Context<ExecuteEphemeralAuthorization>,
        instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
        split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
        vault_index: u8,
    ) -> Result<()> {
        instructions::execute_ephemeral_authorization(
            ctx,
            instruction_data_list,
            split_index,
            vault_index,
        )
    }
}
