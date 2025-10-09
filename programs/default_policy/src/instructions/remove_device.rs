// use crate::{error::PolicyError, state::Policy, ID};
// use anchor_lang::prelude::*;
// use lazorkit::{
//     constants::{PASSKEY_PUBLIC_KEY_SIZE, SMART_WALLET_SEED},
//     state::WalletDevice,
//     utils::PasskeyExt as _,
//     ID as LAZORKIT_ID,
// };

// pub fn remove_device(
//     ctx: Context<RemoveDevice>,
//     wallet_id: u64,
//     passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
//     remove_passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
// ) -> Result<()> {
//     let wallet_device = &mut ctx.accounts.wallet_device;
//     let smart_wallet = &mut ctx.accounts.smart_wallet;
//     let rm_wallet_device = &mut ctx.accounts.rm_wallet_device;

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

//     let expected_rm_wallet_device_pubkey = Pubkey::find_program_address(
//         &[
//             WalletDevice::PREFIX_SEED,
//             expected_smart_wallet_pubkey.as_ref(),
//             remove_passkey_public_key
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
//         rm_wallet_device.key() == expected_rm_wallet_device_pubkey,
//         PolicyError::Unauthorized
//     );

//     let policy = &mut ctx.accounts.policy;

//     // check if the rm wallet device is in the list
//     if !policy.list_wallet_device.contains(&rm_wallet_device.key()) {
//         return err!(PolicyError::WalletDeviceNotInPolicy);
//     }

//     let position = policy
//         .list_wallet_device
//         .iter()
//         .position(|k| k == &rm_wallet_device.key())
//         .unwrap();
//     policy.list_wallet_device.remove(position);

//     Ok(())
// }

// #[derive(Accounts)]
// pub struct RemoveDevice<'info> {
//     #[account(mut)]
//     pub smart_wallet: SystemAccount<'info>,

//     #[account(
//         owner = LAZORKIT_ID,
//         signer,
//     )]
//     pub wallet_device: Account<'info, WalletDevice>,

//     /// CHECK:
//     #[account(mut)]
//     pub rm_wallet_device: UncheckedAccount<'info>,

//     #[account(
//         mut,
//         seeds = [Policy::PREFIX_SEED, smart_wallet.key().as_ref()],
//         bump,
//         owner = ID,
//         constraint = policy.list_wallet_device.contains(&wallet_device.key()) @ PolicyError::Unauthorized,
//         constraint = policy.smart_wallet == smart_wallet.key() @ PolicyError::Unauthorized,
//     )]
//     pub policy: Account<'info, Policy>,
// }
