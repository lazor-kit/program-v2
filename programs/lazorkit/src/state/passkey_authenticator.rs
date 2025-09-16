use crate::{
    constants::PASSKEY_SIZE, error::LazorKitError, state::BpfWriter, utils::PasskeyExt as _, ID,
};
use anchor_lang::{
    prelude::*,
    system_program::{create_account, CreateAccount},
};

/// Account that stores a passkey authenticator used to authenticate to a smart wallet
#[account]
#[derive(Debug, InitSpace)]
pub struct PasskeyAuthenticator {
    /// The public key of the passkey for this authenticator that can authorize transactions
    pub passkey_pubkey: [u8; PASSKEY_SIZE],
    /// The smart wallet this authenticator belongs to
    pub smart_wallet: Pubkey,

    /// The credential ID this authenticator belongs to
    #[max_len(256)]
    pub credential_id: Vec<u8>,

    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl PasskeyAuthenticator {
    pub const PREFIX_SEED: &'static [u8] = b"passkey_authenticator";

    fn from<'info>(x: &'info AccountInfo<'info>) -> Account<'info, Self> {
        Account::try_from_unchecked(x).unwrap()
    }

    fn serialize(&self, info: AccountInfo) -> anchor_lang::Result<()> {
        let dst: &mut [u8] = &mut info.try_borrow_mut_data().unwrap();
        let mut writer: BpfWriter<&mut [u8]> = BpfWriter::new(dst);
        PasskeyAuthenticator::try_serialize(self, &mut writer)
    }

    pub fn init<'info>(
        passkey_authenticator: &'info AccountInfo<'info>,
        payer: AccountInfo<'info>,
        system_program: AccountInfo<'info>,
        smart_wallet: Pubkey,
        passkey_pubkey: [u8; PASSKEY_SIZE],
        credential_id: Vec<u8>,
    ) -> Result<()> {
        let a = passkey_pubkey.to_hashed_bytes(smart_wallet);
        if passkey_authenticator.data_is_empty() {
            // Create the seeds and bump for PDA address calculation
            let seeds: &[&[u8]] = &[PasskeyAuthenticator::PREFIX_SEED, smart_wallet.as_ref(), a.as_ref()];
            let (_, bump) = Pubkey::find_program_address(&seeds, &ID);
            let seeds_signer = &mut seeds.to_vec();
            let binding = [bump];
            seeds_signer.push(&binding);

            let space: u64 = (8 + PasskeyAuthenticator::INIT_SPACE) as u64;

            // Create account if it doesn't exist
            create_account(
                CpiContext::new(
                    system_program,
                    CreateAccount {
                        from: payer,
                        to: passkey_authenticator.clone(),
                    },
                )
                .with_signer(&[seeds_signer]),
                Rent::get()?.minimum_balance(space.try_into().unwrap()),
                space,
                &ID,
            )?;

            let mut auth = PasskeyAuthenticator::from(passkey_authenticator);

            auth.set_inner(PasskeyAuthenticator {
                passkey_pubkey,
                smart_wallet,
                credential_id,
                bump,
            });
            auth.serialize(auth.to_account_info())
        } else {
            return err!(LazorKitError::PasskeyAuthenticatorAlreadyInitialized);
        }
    }
}
