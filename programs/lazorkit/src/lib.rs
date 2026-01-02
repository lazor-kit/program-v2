use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod security;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh");

/// LazorKit: Smart Wallet with WebAuthn Passkey Authentication
#[program]
pub mod lazorkit {
    use super::*;

    pub fn create_smart_wallet(
        ctx: Context<CreateSmartWallet>,
        args: CreateSmartWalletArgs,
    ) -> Result<()> {
        instructions::create_smart_wallet(ctx, args)
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
}
