use anchor_lang::prelude::*;
use lazorkit::constants::PASSKEY_PUBLIC_KEY_SIZE;

#[derive(Debug, AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Clone, Copy)]
pub struct DeviceSlot {
    pub passkey_pubkey: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub credential_hash: [u8; 32],
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct PolicyStruct {
    pub bump: u8,
    pub smart_wallet: Pubkey,
    pub device_slots: Vec<DeviceSlot>,
}
