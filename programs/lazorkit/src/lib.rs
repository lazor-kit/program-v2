use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod security;
pub mod state;
pub mod utils;

use instructions::*;
use state::*;

declare_id!("G5SuNc9zcsxi2ANAy13XweXaczWxq2vzJCFz3pmVEqNJ");

/// LazorKit: Enterprise Smart Wallet with WebAuthn Passkey Authentication
///
/// LazorKit is a comprehensive smart wallet solution that enables secure, user-friendly
/// transaction execution using WebAuthn passkey authentication. The program provides:
///
/// - **Passkey-based Authentication**: Secure transaction signing using WebAuthn standards
/// - **Policy-driven Security**: Configurable transaction validation through policy programs
/// - **Chunked Transactions**: Support for large transactions via chunked execution
/// - **Permission System**: Ephemeral key grants for enhanced user experience
/// - **Vault Management**: Multi-slot fee distribution and SOL management
/// - **Admin Controls**: Program configuration and policy program registration
///
/// The program is designed for enterprise use cases requiring high security, scalability,
/// and user experience while maintaining compatibility with existing Solana infrastructure.
#[program]
pub mod lazorkit {
    use super::*;

    /// Initialize the LazorKit program with essential configuration
    ///
    /// Sets up the program's initial state including the sequence tracker for transaction
    /// ordering and default configuration parameters. This must be called before any
    /// other operations can be performed.
    pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
        instructions::initialize_program(ctx)
    }

    /// Update program configuration settings
    ///
    /// Allows the program authority to modify critical configuration parameters including
    /// fee structures, default policy programs, and operational settings. This function
    /// supports updating various configuration types through the UpdateType enum.
    pub fn update_config(ctx: Context<UpdateConfig>, param: UpdateType, value: u64) -> Result<()> {
        instructions::update_config(ctx, param, value)
    }

    /// Create a new smart wallet with WebAuthn passkey authentication
    ///
    /// Creates a new smart wallet account with associated passkey device for secure
    /// authentication. The wallet is initialized with the specified policy program
    /// for transaction validation and can receive initial SOL funding.
    pub fn create_smart_wallet(
        ctx: Context<CreateSmartWallet>,
        args: CreateSmartWalletArgs,
    ) -> Result<()> {
        instructions::create_smart_wallet(ctx, args)
    }

    /// Register a new policy program in the whitelist
    ///
    /// Allows the program authority to add new policy programs to the registry.
    /// These policy programs can then be used by smart wallets for transaction
    /// validation and security enforcement.
    pub fn add_policy_program(ctx: Context<RegisterPolicyProgram>) -> Result<()> {
        instructions::add_policy_program(ctx)
    }

    /// Change the policy program for a smart wallet
    ///
    /// Allows changing the policy program that governs a smart wallet's transaction
    /// validation rules. Requires proper passkey authentication and validates that
    /// the new policy program is registered in the whitelist.
    pub fn change_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, ChangePolicy<'info>>,
        args: ChangePolicyArgs,
    ) -> Result<()> {
        instructions::change_policy(ctx, args)
    }

    /// Execute policy program instructions
    ///
    /// Calls the policy program to perform operations like adding/removing devices
    /// or other policy-specific actions. Requires proper passkey authentication
    /// and validates the policy program is registered.
    pub fn call_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CallPolicy<'info>>,
        args: CallPolicyArgs,
    ) -> Result<()> {
        instructions::call_policy(ctx, args)
    }

    /// Execute a transaction through the smart wallet
    ///
    /// The main transaction execution function that validates the transaction through
    /// the policy program before executing the target program instruction. Supports
    /// complex multi-instruction transactions with proper authentication.
    pub fn execute<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
        args: ExecuteArgs,
    ) -> Result<()> {
        instructions::execute(ctx, args)
    }

    /// Create a chunk buffer for large transactions
    ///
    /// Creates a buffer for chunked transactions when the main execute transaction
    /// exceeds size limits. Splits large transactions into smaller, manageable
    /// chunks that can be processed separately while maintaining security.
    pub fn create_chunk<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CreateChunk<'info>>,
        args: CreateChunkArgs,
    ) -> Result<()> {
        instructions::create_chunk(ctx, args)
    }

    /// Execute a chunk from the chunk buffer
    ///
    /// Executes a chunk from the previously created buffer. Used when the main
    /// execute transaction is too large and needs to be split into smaller,
    /// manageable pieces for processing.
    pub fn execute_chunk(
        ctx: Context<ExecuteChunk>,
        instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
        split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
    ) -> Result<()> {
        instructions::execute_chunk(ctx, instruction_data_list, split_index)
    }

    /// Manage SOL transfers in the vault system
    ///
    /// Handles SOL transfers to and from the LazorKit vault system, supporting
    /// multiple vault slots for efficient fee distribution and program operations.
    pub fn manage_vault(
        ctx: Context<ManageVault>,
        action: u8,
        amount: u64,
        index: u8,
    ) -> Result<()> {
        instructions::manage_vault(ctx, action, amount, index)
    }

    /// Grant ephemeral permission to a keypair
    ///
    /// Grants time-limited permission to an ephemeral keypair to interact with
    /// the smart wallet. Ideal for games or applications that need to perform
    /// multiple operations without repeatedly authenticating with passkey.
    pub fn grant_permission<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, GrantPermission<'info>>,
        args: GrantPermissionArgs,
    ) -> Result<()> {
        instructions::grant_permission(ctx, args)
    }

    /// Execute transactions using ephemeral permission
    ///
    /// Executes transactions using a previously granted ephemeral key, allowing
    /// multiple operations without repeated passkey authentication. Perfect for
    /// games or applications that require frequent interactions with the wallet.
    pub fn execute_with_permission(
        ctx: Context<ExecuteWithPermission>,
        instruction_data_list: Vec<Vec<u8>>, // Multiple instruction data
        split_index: Vec<u8>,                // Split indices for accounts (n-1 for n instructions)
    ) -> Result<()> {
        instructions::execute_with_permission(ctx, instruction_data_list, split_index)
    }
}
