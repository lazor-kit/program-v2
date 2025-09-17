use crate::{
    constants::PASSKEY_PUBLIC_KEY_SIZE, error::LazorKitError, state::BpfWriter, utils::PasskeyExt as _, ID,
};
use anchor_lang::{
    prelude::*,
    system_program::{create_account, CreateAccount},
};

/// Account that stores a wallet device (passkey) used to authenticate to a smart wallet
#[account]
#[derive(Debug, InitSpace)]
pub struct WalletDevice {
    /// The public key of the passkey for this wallet device that can authorize transactions
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    /// The smart wallet this wallet device belongs to
    pub smart_wallet_address: Pubkey,

    /// The credential ID this wallet device belongs to
    #[max_len(256)]
    pub credential_id: Vec<u8>,

    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl WalletDevice {
    pub const PREFIX_SEED: &'static [u8] = b"wallet_device";

    fn from<'info>(x: &'info AccountInfo<'info>) -> Account<'info, Self> {
        Account::try_from_unchecked(x).unwrap()
    }

    fn serialize(&self, info: AccountInfo) -> anchor_lang::Result<()> {
        let dst: &mut [u8] = &mut info.try_borrow_mut_data().unwrap();
        let mut writer: BpfWriter<&mut [u8]> = BpfWriter::new(dst);
        WalletDevice::try_serialize(self, &mut writer)
    }

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
                Rent::get()?.minimum_balance(space.try_into().unwrap()),
                space,
                &ID,
            )?;

            let mut auth = WalletDevice::from(wallet_device);

            auth.set_inner(WalletDevice {
                passkey_public_key,
                smart_wallet_address,
                credential_id,
                bump,
            });
            auth.serialize(auth.to_account_info())
        } else {
            return err!(LazorKitError::WalletDeviceAlreadyInitialized);
        }
    }
}
