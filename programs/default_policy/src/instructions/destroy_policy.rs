use crate::{error::PolicyError, state::Policy, ID};
use anchor_lang::prelude::*;
use lazorkit::{
    constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
    state::WalletDevice,
    utils::PasskeyExt as _,
    ID as LAZORKIT_ID,
};

pub fn destroy_policy(
    ctx: Context<DestroyPolicy>,
    wallet_id: u64,
    passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
) -> Result<()> {
    let wallet_device = &mut ctx.accounts.wallet_device;
    let smart_wallet = &mut ctx.accounts.smart_wallet;

    let expected_smart_wallet_pubkey = Pubkey::find_program_address(
        &[SMART_WALLET_SEED, wallet_id.to_le_bytes().as_ref()],
        &LAZORKIT_ID,
    )
    .0;

    let expected_wallet_device_pubkey = Pubkey::find_program_address(
        &[
            WalletDevice::PREFIX_SEED,
            expected_smart_wallet_pubkey.as_ref(),
            passkey_public_key
                .to_hashed_bytes(expected_smart_wallet_pubkey)
                .as_ref(),
        ],
        &LAZORKIT_ID,
    )
    .0;

    require!(
        smart_wallet.key() == expected_smart_wallet_pubkey,
        PolicyError::Unauthorized
    );
    require!(
        wallet_device.key() == expected_wallet_device_pubkey,
        PolicyError::Unauthorized
    );

    Ok(())
}

#[derive(Accounts)]
pub struct DestroyPolicy<'info> {
    #[account(mut)]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        owner = LAZORKIT_ID,
        signer,
    )]
    pub wallet_device: Account<'info, WalletDevice>,

    /// CHECK:
    #[account(mut)]
    pub new_wallet_device: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [Policy::PREFIX_SEED, wallet_device.key().as_ref()],
        bump,
        owner = ID,
        constraint = policy.list_wallet_device.contains(&wallet_device.key()) @ PolicyError::Unauthorized,
        constraint = policy.smart_wallet == smart_wallet.key() @ PolicyError::Unauthorized,
        close = smart_wallet,
    )]
    pub policy: Account<'info, Policy>,
}
