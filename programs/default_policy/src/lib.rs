use anchor_lang::prelude::*;

declare_id!("BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7");

mod error;
mod instructions;
mod state;

use instructions::*;
use state::*;

#[constant]
pub const POLICY_DATA_SIZE: u16 = 32 + 4 + 32 * 5; // PolicyStruct::INIT_SPACE

#[program]
pub mod default_policy {
    use super::*;
    pub fn init_policy(ctx: Context<InitPolicy>) -> Result<PolicyStruct> {
        instructions::init_policy(ctx)
    }

    pub fn check_policy(ctx: Context<CheckPolicy>, policy_data: Vec<u8>) -> Result<()> {
        instructions::check_policy(ctx, policy_data)
    }

    pub fn add_authority(
        ctx: Context<AddAuthority>,
        policy_data: Vec<u8>,
        new_authority: Pubkey,
    ) -> Result<PolicyStruct> {
        instructions::add_authority(ctx, policy_data, new_authority)
    }

    pub fn remove_authority(
        ctx: Context<RemoveAuthority>,
        policy_data: Vec<u8>,
        new_authority: Pubkey,
    ) -> Result<PolicyStruct> {
        instructions::remove_authority(ctx, policy_data, new_authority)
    }
}
