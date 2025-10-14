use anchor_lang::prelude::*;
use lazorkit::{
    constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
    state::DeviceSlot,
    utils::hash_seeds,
    ID as LAZORKIT_ID,
};

use crate::{error::PolicyError, state::PolicyStruct};

pub fn check_policy(
    ctx: Context<CheckPolicy>,
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

    let hashed = hash_seeds(&passkey_public_key, smart_wallet.key());

    let expected_policy_signer_pubkey = Pubkey::find_program_address(&[&hashed], &LAZORKIT_ID).0;

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

    /// CHECK: bound via constraint to policy.smart_wallet
    pub smart_wallet: SystemAccount<'info>,
}
