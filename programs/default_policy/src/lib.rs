use anchor_lang::prelude::*;

declare_id!("CNT2aEgxucQjmt5SRsA6hSGrt241Bvc9zsgPvSuMjQTE");

mod error;
mod instructions;
mod state;

use instructions::*;
use lazorkit::constants::PASSKEY_PUBLIC_KEY_SIZE;

#[program]
pub mod default_policy {

    use super::*;

    pub fn init_policy(
        ctx: Context<InitPolicy>,
        wallet_id: u64,
        passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    ) -> Result<()> {
        instructions::init_policy(ctx, wallet_id, passkey_public_key)
    }

    pub fn check_policy(
        ctx: Context<CheckPolicy>,
        wallet_id: u64,
        passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    ) -> Result<()> {
        instructions::check_policy(ctx, wallet_id, passkey_public_key)
    }

    pub fn add_device(ctx: Context<AddDevice>) -> Result<()> {
        instructions::add_device(ctx)
    }
}
