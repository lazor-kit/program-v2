use anchor_lang::prelude::*;

declare_id!("BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7");

mod error;
mod instructions;
mod state;

use instructions::*;
use lazorkit::constants::PASSKEY_PUBLIC_KEY_SIZE;
use state::*;

#[program]
pub mod default_policy {

    use super::*;

    pub fn init_policy(
        ctx: Context<InitPolicy>,
        wallet_id: u64,
        passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
        credential_hash: [u8; 32],
    ) -> Result<PolicyStruct> {
        instructions::init_policy(ctx, wallet_id, passkey_public_key, credential_hash)
    }

    pub fn check_policy(
        ctx: Context<CheckPolicy>,
        wallet_id: u64,
        passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
        credential_hash: [u8; 32],
        policy_data: Vec<u8>,
    ) -> Result<()> {
        instructions::check_policy(
            ctx,
            wallet_id,
            passkey_public_key,
            credential_hash,
            policy_data,
        )
    }

    // pub fn add_device(
    //     ctx: Context<AddDevice>,
    //     wallet_id: u64,
    //     passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    //     new_passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    // ) -> Result<()> {
    //     instructions::add_device(ctx, wallet_id, passkey_public_key, new_passkey_public_key)
    // }

    // pub fn remove_device(
    //     ctx: Context<RemoveDevice>,
    //     wallet_id: u64,
    //     passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    //     remove_passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    // ) -> Result<()> {
    //     instructions::remove_device(
    //         ctx,
    //         wallet_id,
    //         passkey_public_key,
    //         remove_passkey_public_key,
    //     )
    // }

    // pub fn destroy_policy(
    //     ctx: Context<DestroyPolicy>,
    //     wallet_id: u64,
    //     passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    // ) -> Result<()> {
    //     instructions::destroy_policy(ctx, wallet_id, passkey_public_key)
    // }
}
