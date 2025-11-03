use anchor_lang::prelude::*;
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

pub fn destroy_policy(
    ctx: Context<DestroyPolicy>,
    wallet_id: u64,
    passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    credential_hash: [u8; 32],
    policy_data: Vec<u8>,
) -> Result<()> {
    let policy_signer = &mut ctx.accounts.policy_signer;
    let smart_wallet = &mut ctx.accounts.smart_wallet;

    let expected_smart_wallet_pubkey = Pubkey::find_program_address(
        &[SMART_WALLET_SEED, wallet_id.to_le_bytes().as_ref()],
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
        policy_signer.key() == expected_policy_signer_pubkey,
        PolicyError::Unauthorized
    );

    let policy_struct = PolicyStruct::try_from_slice(&policy_data)?;

    require!(
        policy_struct.smart_wallet == smart_wallet.key(),
        PolicyError::Unauthorized
    );

    // Check if the passkey public key is in the device slots
    let device_slots = policy_struct.device_slots;
    let device_slot = DeviceSlot {
        passkey_pubkey: passkey_public_key,
        credential_hash: credential_hash,
    };

    require!(
        device_slots.contains(&device_slot),
        PolicyError::Unauthorized
    );

    Ok(())
}

#[derive(Accounts)]
pub struct DestroyPolicy<'info> {
    pub policy_signer: Signer<'info>,

    /// CHECK: bound via constraint to policy.smart_wallet
    pub smart_wallet: SystemAccount<'info>,
}
