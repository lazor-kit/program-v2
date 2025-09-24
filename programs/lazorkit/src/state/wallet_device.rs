use crate::{
    constants::PASSKEY_PUBLIC_KEY_SIZE, error::LazorKitError, state::BpfWriter, utils::PasskeyExt as _, ID,
};
use anchor_lang::{
    prelude::*,
    system_program::{create_account, CreateAccount},
};

/// Account that stores a wallet device (passkey) for smart wallet authentication
///
/// Each wallet device represents a WebAuthn passkey that can be used to authenticate
/// transactions for a specific smart wallet. Multiple devices can be associated with
/// a single smart wallet for enhanced security and convenience.
///
/// Memory layout optimized for better cache performance:
/// - Group related fields together
/// - Align fields to natural boundaries
/// - Minimize padding
#[account]
#[derive(Debug, InitSpace)]
pub struct WalletDevice {
    /// Bump seed for PDA derivation and verification (1 byte)
    pub bump: u8,
    /// Padding to align next fields (7 bytes)
    pub _padding: [u8; 7],
    /// Public key of the WebAuthn passkey for transaction authorization (33 bytes)
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// Smart wallet address this device is associated with (32 bytes)
    pub smart_wallet_address: Pubkey,
    /// Unique credential ID from WebAuthn registration (variable length, max 256 bytes)
    #[max_len(256)]
    pub credential_id: Vec<u8>,
}

impl WalletDevice {
    /// Seed prefix used for PDA derivation of wallet device accounts
    pub const PREFIX_SEED: &'static [u8] = b"wallet_device";

    fn from<'info>(x: &'info AccountInfo<'info>) -> Result<Account<'info, Self>> {
        Account::try_from_unchecked(x).map_err(|_| crate::error::LazorKitError::InvalidAccountData.into())
    }

    fn serialize(&self, info: AccountInfo) -> anchor_lang::Result<()> {
        let dst: &mut [u8] = &mut info.try_borrow_mut_data()
            .map_err(|_| crate::error::LazorKitError::InvalidAccountData)?;
        let mut writer: BpfWriter<&mut [u8]> = BpfWriter::new(dst);
        WalletDevice::try_serialize(self, &mut writer)
    }

    /// Initialize a new wallet device account with passkey credentials
    ///
    /// Creates a new wallet device account that can be used to authenticate
    /// transactions for the specified smart wallet using WebAuthn passkey.
    pub fn init<'info>(
        wallet_device: &'info AccountInfo<'info>,
        payer: AccountInfo<'info>,
        system_program: AccountInfo<'info>,
        smart_wallet_address: Pubkey,
        passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
        credential_id: Vec<u8>,
    ) -> Result<()> {
        let a = passkey_public_key.to_hashed_bytes(smart_wallet_address);
        if wallet_device.data_is_empty() {
            // Create the seeds and bump for PDA address calculation
            let seeds: &[&[u8]] = &[WalletDevice::PREFIX_SEED, smart_wallet_address.as_ref(), a.as_ref()];
            let (_, bump) = Pubkey::find_program_address(&seeds, &ID);
            let seeds_signer = &mut seeds.to_vec();
            let binding = [bump];
            seeds_signer.push(&binding);

            let space: u64 = (8 + WalletDevice::INIT_SPACE) as u64;

            // Create account if it doesn't exist
            create_account(
                CpiContext::new(
                    system_program,
                    CreateAccount {
                        from: payer,
                        to: wallet_device.clone(),
                    },
                )
                .with_signer(&[seeds_signer]),
                Rent::get()?.minimum_balance(space.try_into()
                    .map_err(|_| crate::error::LazorKitError::InvalidAccountData)?),
                space,
                &ID,
            )?;

            let mut auth = WalletDevice::from(wallet_device)?;

            auth.set_inner(WalletDevice {
                bump,
                _padding: [0u8; 7],
                passkey_public_key,
                smart_wallet_address,
                credential_id,
            });
            auth.serialize(auth.to_account_info())
        } else {
            return err!(LazorKitError::WalletDeviceAlreadyInitialized);
        }
    }
}
