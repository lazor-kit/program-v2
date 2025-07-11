use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;

use constants::PASSKEY_SIZE;
use instructions::*;

declare_id!("6Jh4kA4rkZquv9XofKqgbyrRcTDF19uM5HL4xyh6gaSo");

/// The Lazor Kit program provides smart wallet functionality with passkey authentication
#[program]
pub mod lazorkit {
    use super::*;

    /// Initialize the program by creating the sequence tracker
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize(ctx)
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

    /// Spend or mint tokens from the smart wallet after rule check and passkey auth
    pub fn execute_transaction(
        ctx: Context<ExecuteTransaction>,
        args: ExecuteTransactionArgs,
    ) -> Result<()> {
        instructions::execute_transaction(ctx, args)
    }

    /// Swap the rule program associated with the smart wallet
    pub fn update_rule_program(
        ctx: Context<UpdateRuleProgram>,
        args: UpdateRuleProgramArgs,
    ) -> Result<()> {
        instructions::update_rule_program(ctx, args)
    }

    /// Call an arbitrary instruction inside the rule program (and optionally create a new authenticator)
    pub fn call_rule_program(
        ctx: Context<CallRuleProgram>,
        args: CallRuleProgramArgs,
    ) -> Result<()> {
        instructions::call_rule_program(ctx, args)
    }

    /// Update the list of whitelisted rule programs
    pub fn upsert_whitelist_rule_programs(
        ctx: Context<UpsertWhitelistRulePrograms>,
        program_id: Pubkey,
    ) -> Result<()> {
        instructions::upsert_whitelist_rule_programs(ctx, program_id)
    }
}
