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

/// The LazorKit program provides enterprise-grade smart wallet functionality with WebAuthn passkey authentication
///
/// This program enables users to create and manage smart wallets using passkey-based authentication,
/// providing secure transaction execution with configurable policy enforcement and fee distribution.
#[program]
pub mod lazorkit {
    use super::*;

    /// Initialize the LazorKit program with essential configuration
    ///
    /// This function sets up the program's initial state including the sequence tracker
    /// and default configuration parameters.
    pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
        instructions::initialize_program(ctx)
    }

    /// Update program settings
    ///
    /// Only the program authority can call this function to modify configuration
    /// such as fees, default policy programs, and operational parameters.
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        param: UpdateType,
        value: u64,
    ) -> Result<()> {
        instructions::update_config(ctx, param, value)
    }

    /// Create a new smart wallet with WebAuthn passkey authentication
    ///
    /// This function creates a new smart wallet account with associated passkey device,
    /// initializes the wallet with the specified policy program, and transfers initial SOL.
    pub fn create_smart_wallet(
        ctx: Context<CreateSmartWallet>,
        args: CreateSmartWalletArgs,
    ) -> Result<()> {
        instructions::create_smart_wallet(ctx, args)
    }

    /// Add policy program to whitelist
    ///
    /// Only the program authority can add new policy programs to the registry
    /// that can be used by smart wallets for transaction validation.
    pub fn add_policy_program(ctx: Context<RegisterPolicyProgram>) -> Result<()> {
        instructions::add_policy_program(ctx)
    }

    /// Change wallet policy
    ///
    /// This function allows changing the policy program that governs a smart wallet's
    /// transaction validation rules, requiring proper passkey authentication.
    pub fn change_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, ChangePolicy<'info>>,
        args: ChangePolicyArgs,
    ) -> Result<()> {
        instructions::change_policy(ctx, args)
    }

    /// Call policy program
    ///
    /// This function calls the policy program to perform operations like
    /// adding devices, removing devices, or other policy-specific actions.
    pub fn call_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CallPolicy<'info>>,
        args: CallPolicyArgs,
    ) -> Result<()> {
        instructions::call_policy(ctx, args)
    }

    /// Execute transaction
    ///
    /// This is the main transaction execution function that validates the transaction
    /// through the policy program before executing the target program instruction.
    pub fn execute<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
        args: ExecuteArgs,
    ) -> Result<()> {
        instructions::execute(ctx, args)
    }

    /// Create chunk buffer
    ///
    /// This function creates a buffer for chunked transactions when the main
    /// execute transaction is too large. It splits large transactions into
    /// smaller chunks that can be processed separately.
    pub fn create_chunk<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CreateChunk<'info>>,
        args: CreateChunkArgs,
    ) -> Result<()> {
        instructions::create_chunk(ctx, args)
    }

    /// Execute chunk
    ///
    /// This function executes a chunk from the previously created buffer.
    /// Used when the main execute transaction is too large and needs to be
    /// split into smaller, manageable pieces.
    pub fn execute_chunk(
        ctx: Context<ExecuteChunk>,
        instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
        split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
    ) -> Result<()> {
        instructions::execute_chunk(ctx, instruction_data_list, split_index)
    }

    /// Manage vault
    ///
    /// This function handles SOL transfers to and from the LazorKit vault system,
    /// supporting multiple vault slots for efficient fee distribution.
    pub fn manage_vault(
        ctx: Context<ManageVault>,
        action: u8,
        amount: u64,
        index: u8,
    ) -> Result<()> {
        instructions::manage_vault(ctx, action, amount, index)
    }

    /// Grant permission
    ///
    /// This function grants permission to an ephemeral keypair to interact with
    /// the smart wallet for a limited time. Useful for games or apps that need
    /// to perform multiple operations without repeatedly signing with passkey.
    pub fn grant_permission<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, GrantPermission<'info>>,
        args: GrantPermissionArgs,
    ) -> Result<()> {
        instructions::grant_permission(ctx, args)
    }

    /// Execute with permission
    ///
    /// This function executes transactions using a previously granted ephemeral key,
    /// allowing multiple operations without repeated passkey authentication.
    /// Perfect for games or apps that need frequent interactions.
    pub fn execute_with_permission(
        ctx: Context<ExecuteWithPermission>,
        instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
        split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
        vault_index: u8,
    ) -> Result<()> {
        instructions::execute_with_permission(ctx, instruction_data_list, split_index, vault_index)
    }
}
