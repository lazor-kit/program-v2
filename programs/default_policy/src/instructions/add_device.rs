// use crate::{error::PolicyError, state::Policy, ID};
// use anchor_lang::prelude::*;
// use lazorkit::{
//     constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
//     state::WalletDevice,
//     utils::PasskeyExt as _,
//     ID as LAZORKIT_ID,
// };

// pub fn add_device(
//     ctx: Context<AddDevice>,
//     wallet_id: u64,
//     passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
//     new_passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
// ) -> Result<()> {
//     let wallet_device = &mut ctx.accounts.wallet_device;
//     let smart_wallet = &mut ctx.accounts.smart_wallet;
//     let new_wallet_device = &mut ctx.accounts.new_wallet_device;

//     let expected_smart_wallet_pubkey = Pubkey::find_program_address(
//         &[SMART_WALLET_SEED, wallet_id.to_le_bytes().as_ref()],
//         &LAZORKIT_ID,
//     )
//     .0;

//     let expected_wallet_device_pubkey = Pubkey::find_program_address(
//         &[
//             WalletDevice::PREFIX_SEED,
//             expected_smart_wallet_pubkey.as_ref(),
//             passkey_public_key
//                 .to_hashed_bytes(expected_smart_wallet_pubkey)
//                 .as_ref(),
//         ],
//         &LAZORKIT_ID,
//     )
//     .0;

//     let expected_new_wallet_device_pubkey = Pubkey::find_program_address(
//         &[
//             WalletDevice::PREFIX_SEED,
//             expected_smart_wallet_pubkey.as_ref(),
//             new_passkey_public_key
//                 .to_hashed_bytes(expected_smart_wallet_pubkey)
//                 .as_ref(),
//         ],
//         &LAZORKIT_ID,
//     )
//     .0;

//     require!(
//         smart_wallet.key() == expected_smart_wallet_pubkey,
//         PolicyError::Unauthorized
//     );
//     require!(
//         wallet_device.key() == expected_wallet_device_pubkey,
//         PolicyError::Unauthorized
//     );

//     require!(
//         new_wallet_device.key() == expected_new_wallet_device_pubkey,
//         PolicyError::Unauthorized
//     );

//     let policy = &mut ctx.accounts.policy;
//     // check if the new wallet device is already in the list
//     if policy.list_wallet_device.contains(&new_wallet_device.key()) {
//         return err!(PolicyError::WalletDeviceAlreadyInPolicy);
//     }
//     policy.list_wallet_device.push(new_wallet_device.key());

//     Ok(())
// }

// #[derive(Accounts)]
// pub struct AddDevice<'info> {
//     #[account(mut)]
//     pub smart_wallet: SystemAccount<'info>,

//     #[account(
//         owner = LAZORKIT_ID,
//         signer,
//     )]
//     pub wallet_device: Account<'info, WalletDevice>,

//     /// CHECK:
//     #[account(mut)]
//     pub new_wallet_device: UncheckedAccount<'info>,

//     #[account(
//         mut,
//         seeds = [Policy::PREFIX_SEED, smart_wallet.key().as_ref()],
//         bump,
//         owner = ID,
//         constraint = policy.list_wallet_device.contains(&wallet_device.key()) @ PolicyError::Unauthorized,
//     )]
//     pub policy: Account<'info, Policy>,
// }
