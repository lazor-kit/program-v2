use crate::{error::PolicyError, state::Policy};
use anchor_lang::prelude::*;
use lazorkit::{
    constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
    program::Lazorkit,
    state::WalletDevice,
    utils::PasskeyExt as _,
    ID as LAZORKIT_ID,
};

pub fn init_policy(
    ctx: Context<InitPolicy>,
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

    let policy = &mut ctx.accounts.policy;

    policy.smart_wallet = ctx.accounts.smart_wallet.key();
    policy.wallet_device = ctx.accounts.wallet_device.key();

    Ok(())
}

#[derive(Accounts)]
pub struct InitPolicy<'info> {
    /// CHECK:
    #[account(mut, signer)]
    pub smart_wallet: SystemAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub wallet_device: UncheckedAccount<'info>,

    #[account(
        init,
        payer = smart_wallet,
        space = 8 + Policy::INIT_SPACE,
        seeds = [Policy::PREFIX_SEED, wallet_device.key().as_ref()],
        bump,
    )]
    pub policy: Account<'info, Policy>,

    pub lazorkit: Program<'info, Lazorkit>,

    pub system_program: Program<'info, System>,
}
