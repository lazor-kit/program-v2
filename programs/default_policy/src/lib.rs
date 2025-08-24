use anchor_lang::prelude::*;

declare_id!("CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE");

mod error;
mod instructions;
mod state;

use instructions::*;

#[program]
pub mod default_policy {

    use super::*;

    pub fn init_policy(ctx: Context<InitPolicy>) -> Result<()> {
        instructions::init_policy(ctx)
    }

    pub fn check_policy(_ctx: Context<CheckPolicy>) -> Result<()> {
        instructions::check_policy(_ctx)
    }

    pub fn add_device(ctx: Context<AddDevice>) -> Result<()> {
        instructions::add_device(ctx)
    }
}
