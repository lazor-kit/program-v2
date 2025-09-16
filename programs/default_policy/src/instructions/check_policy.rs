use anchor_lang::prelude::*;
use lazorkit::{
    constants::{PASSKEY_SIZE, SMART_WALLET_SEED},
    state::WalletDevice,
    utils::PasskeyExt as _,
    ID as LAZORKIT_ID,
};

use crate::{error::PolicyError, state::Policy, ID};

pub fn check_policy(
    ctx: Context<CheckPolicy>,
    wallet_id: u64,
    passkey_public_key: [u8; PASSKEY_SIZE],
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
pub struct CheckPolicy<'info> {
    pub wallet_device: Signer<'info>,
    /// CHECK: bound via constraint to policy.smart_wallet
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        owner = ID,
        constraint = wallet_device.key() == policy.wallet_device @ PolicyError::Unauthorized,
        constraint = policy.smart_wallet == smart_wallet.key() @ PolicyError::Unauthorized,
    )]
    pub policy: Account<'info, Policy>,
}
