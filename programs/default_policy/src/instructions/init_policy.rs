use crate::{
    error::PolicyError,
    state::{DeviceSlot, PolicyStruct},
};
use anchor_lang::{prelude::*, solana_program::hash::HASH_BYTES};
use lazorkit::{
    constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
    state::{WalletDevice, WalletState},
    utils::create_wallet_device_hash,
    ID as LAZORKIT_ID,
};

/// Initialize policy for a new smart wallet
pub fn init_policy(
    ctx: Context<InitPolicy>,
    wallet_id: u64,
    passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    credential_hash: [u8; HASH_BYTES],
) -> Result<PolicyStruct> {
    let smart_wallet_key = ctx.accounts.smart_wallet.key();
    let wallet_state_key = ctx.accounts.wallet_state.key();
    let policy_signer_key = ctx.accounts.policy_signer.key();

    let (expected_smart_wallet_pubkey, smart_wallet_bump) = Pubkey::find_program_address(
        &[SMART_WALLET_SEED, wallet_id.to_le_bytes().as_ref()],
        &LAZORKIT_ID,
    );

    let expected_wallet_state_pubkey = Pubkey::find_program_address(
        &[WalletState::PREFIX_SEED, smart_wallet_key.as_ref()],
        &LAZORKIT_ID,
    )
    .0;

    let wallet_device_hash = create_wallet_device_hash(smart_wallet_key, credential_hash);
    let expected_policy_signer_pubkey = Pubkey::find_program_address(
        &[WalletDevice::PREFIX_SEED, &wallet_device_hash],
        &LAZORKIT_ID,
    )
    .0;

    require!(
        smart_wallet_key == expected_smart_wallet_pubkey,
        PolicyError::Unauthorized
    );
    require!(
        wallet_state_key == expected_wallet_state_pubkey,
        PolicyError::Unauthorized
    );
    require!(
        policy_signer_key == expected_policy_signer_pubkey,
        PolicyError::Unauthorized
    );

    Ok(PolicyStruct {
        bump: smart_wallet_bump,
        smart_wallet: smart_wallet_key,
        device_slots: vec![DeviceSlot {
            passkey_pubkey: passkey_public_key,
            credential_hash,
        }],
    })
}

#[derive(Accounts)]
pub struct InitPolicy<'info> {
    pub policy_signer: Signer<'info>,

    #[account(mut)]
    pub smart_wallet: SystemAccount<'info>,

    #[account(mut)]
    /// CHECK: Validated via PDA derivation in instruction logic
    pub wallet_state: UncheckedAccount<'info>,
}
