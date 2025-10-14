use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod security;
pub mod state;
pub mod utils;

use instructions::*;
use state::*;

declare_id!("Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh");

/// LazorKit: Smart Wallet with WebAuthn Passkey Authentication
#[program]
pub mod lazorkit {
    use super::*;

    pub fn initialize_program(ctx: Context<InitializeProgram>) -> Result<()> {
        instructions::initialize_program(ctx)
    }

    pub fn update_config(ctx: Context<UpdateConfig>, param: UpdateType, value: u64) -> Result<()> {
        instructions::update_config(ctx, param, value)
    }

    pub fn create_smart_wallet(
        ctx: Context<CreateSmartWallet>,
        args: CreateSmartWalletArgs,
    ) -> Result<()> {
        instructions::create_smart_wallet(ctx, args)
    }

    pub fn add_policy_program(ctx: Context<RegisterPolicyProgram>) -> Result<()> {
        instructions::add_policy_program(ctx)
    }

    pub fn change_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, ChangePolicy<'info>>,
        args: ChangePolicyArgs,
    ) -> Result<()> {
        instructions::change_policy(ctx, args)
    }

    pub fn call_policy<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CallPolicy<'info>>,
        args: CallPolicyArgs,
    ) -> Result<()> {
        instructions::call_policy(ctx, args)
    }

    pub fn execute<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
        args: ExecuteArgs,
    ) -> Result<()> {
        instructions::execute(ctx, args)
    }

    pub fn create_chunk<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, CreateChunk<'info>>,
        args: CreateChunkArgs,
    ) -> Result<()> {
        instructions::create_chunk(ctx, args)
    }

    pub fn execute_chunk(
        ctx: Context<ExecuteChunk>,
        instruction_data_list: Vec<Vec<u8>>,
        split_index: Vec<u8>,
    ) -> Result<()> {
        instructions::execute_chunk(ctx, instruction_data_list, split_index)
    }

    pub fn close_chunk(ctx: Context<CloseChunk>) -> Result<()> {
        instructions::close_chunk(ctx)
    }

    pub fn manage_vault(
        ctx: Context<ManageVault>,
        action: u8,
        amount: u64,
        index: u8,
    ) -> Result<()> {
        instructions::manage_vault(ctx, action, amount, index)
    }
}
