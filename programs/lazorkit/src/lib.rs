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

declare_id!("HKAM6aGJsNuyxoVKNk8kgqMTUNSDjA3ciZUikHYemQzL");

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
    ) -> Result<()> {
        instructions::create_smart_wallet(ctx, passkey_pubkey, credential_id, rule_data)
    }

    /// Unified execute entrypoint covering all smart-wallet actions
    pub fn execute<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, Execute<'info>>,
        args: ExecuteArgs,
    ) -> Result<()> {
        instructions::execute(ctx, args)
    }

    /// Add a program to the whitelist of rule programs
    pub fn add_whitelist_rule_program(ctx: Context<AddWhitelistRuleProgram>) -> Result<()> {
        instructions::add_whitelist_rule_program(ctx)
    }
}
