use crate::{
    error::PolicyError,
    state::{DeviceSlot, PolicyStruct},
};
use anchor_lang::prelude::*;
use lazorkit::{
    constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
    state::{WalletDevice, WalletState},
    utils::create_wallet_device_hash,
    ID as LAZORKIT_ID,
};

pub fn init_policy(
    ctx: Context<InitPolicy>,
    wallet_id: u64,
    passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    credential_hash: [u8; 32],
) -> Result<PolicyStruct> {
    let smart_wallet = &mut ctx.accounts.smart_wallet;
    let wallet_state = &mut ctx.accounts.wallet_state;
    let policy_signer = &mut ctx.accounts.policy_signer;

    let (expected_smart_wallet_pubkey, smart_wallet_bump) = Pubkey::find_program_address(
        &[SMART_WALLET_SEED, wallet_id.to_le_bytes().as_ref()],
        &LAZORKIT_ID,
    );

    let expected_wallet_state_pubkey = Pubkey::find_program_address(
        &[WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        &LAZORKIT_ID,
    )
    .0;

    let expected_policy_signer_pubkey = Pubkey::find_program_address(
        &[
            WalletDevice::PREFIX_SEED,
            &create_wallet_device_hash(smart_wallet.key(), credential_hash),
        ],
        &LAZORKIT_ID,
    )
    .0;

    require!(
        smart_wallet.key() == expected_smart_wallet_pubkey,
        PolicyError::Unauthorized
    );
    require!(
        wallet_state.key() == expected_wallet_state_pubkey,
        PolicyError::Unauthorized
    );

    require!(
        policy_signer.key() == expected_policy_signer_pubkey,
        PolicyError::Unauthorized
    );

    let return_data: PolicyStruct = PolicyStruct {
        bump: smart_wallet_bump,
        smart_wallet: smart_wallet.key(),
        device_slots: vec![DeviceSlot {
            passkey_pubkey: passkey_public_key,
            credential_hash,
        }],
    };

    Ok(return_data)
}

#[derive(Accounts)]
pub struct InitPolicy<'info> {
    pub policy_signer: Signer<'info>,

    /// CHECK:
    #[account(mut)]
    pub smart_wallet: SystemAccount<'info>,

    #[account(mut)]
    /// CHECK: bound via constraint to smart_wallet
    pub wallet_state: UncheckedAccount<'info>,
}
