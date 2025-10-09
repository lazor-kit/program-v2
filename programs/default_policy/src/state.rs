use anchor_lang::prelude::*;
use lazorkit::state::DeviceSlot;

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct PolicyStruct {
    pub bump: u8,
    pub smart_wallet: Pubkey,
    pub device_slots: Vec<DeviceSlot>,
}
