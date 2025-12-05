use anchor_lang::{prelude::*, solana_program::hash::HASH_BYTES};
use lazorkit::{
    constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
    state::WalletDevice,
    utils::create_wallet_device_hash,
    ID as LAZORKIT_ID,
};

use crate::{
    error::PolicyError,
    state::{DeviceSlot, PolicyStruct},
};

/// Verify that a passkey is authorized for a smart wallet transaction
pub fn check_policy(
    ctx: Context<CheckPolicy>,
    wallet_id: u64,
    passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    credential_hash: [u8; HASH_BYTES],
    policy_data: Vec<u8>,
) -> Result<()> {
    let smart_wallet_key = ctx.accounts.smart_wallet.key();
    let policy_signer_key = ctx.accounts.policy_signer.key();

    let expected_smart_wallet_pubkey = Pubkey::find_program_address(
        &[SMART_WALLET_SEED, wallet_id.to_le_bytes().as_ref()],
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
        policy_signer_key == expected_policy_signer_pubkey,
        PolicyError::Unauthorized
    );

    let policy_struct = PolicyStruct::try_from_slice(&policy_data)?;

    require!(
        policy_struct.smart_wallet == smart_wallet_key,
        PolicyError::Unauthorized
    );

    require!(
        policy_struct.device_slots.contains(&DeviceSlot {
            passkey_pubkey: passkey_public_key,
            credential_hash,
        }),
        PolicyError::Unauthorized
    );

    Ok(())
}

#[derive(Accounts)]
pub struct CheckPolicy<'info> {
    pub policy_signer: Signer<'info>,

    pub smart_wallet: SystemAccount<'info>,
}
