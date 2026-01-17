use lazorkit_state::IntoBytes;
use no_padding::NoPadding;
use pinocchio::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use std::slice;

pub const MAX_WHITELIST_SIZE: usize = 100;

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, NoPadding)]
pub struct WhitelistState {
    pub count: u16,
    pub _padding: [u8; 6],
    pub addresses: [Pubkey; MAX_WHITELIST_SIZE],
}

impl WhitelistState {
    pub const LEN: usize = 2 + 6 + (32 * MAX_WHITELIST_SIZE);
}

impl IntoBytes for WhitelistState {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        Ok(unsafe { slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) })
    }
}

pub struct WhitelistBuilder {
    addresses: Vec<Pubkey>,
}

impl WhitelistBuilder {
    pub fn new() -> Self {
        Self {
            addresses: Vec::new(),
        }
    }

    pub fn add_address(mut self, address: Pubkey) -> Self {
        if self.addresses.len() < MAX_WHITELIST_SIZE {
            self.addresses.push(address);
        }
        self
    }

    pub fn add_addresses(mut self, addresses: &[Pubkey]) -> Self {
        for addr in addresses {
            if self.addresses.len() < MAX_WHITELIST_SIZE {
                self.addresses.push(*addr);
            } else {
                break;
            }
        }
        self
    }

    pub fn build_state(self) -> Vec<u8> {
        let mut fixed_addresses = [Pubkey::default(); MAX_WHITELIST_SIZE];
        for (i, addr) in self.addresses.iter().enumerate() {
            fixed_addresses[i] = *addr;
        }

        let state = WhitelistState {
            count: self.addresses.len() as u16,
            _padding: [0; 6],
            addresses: fixed_addresses,
        };

        let slice = unsafe {
            slice::from_raw_parts(
                &state as *const WhitelistState as *const u8,
                WhitelistState::LEN,
            )
        };
        slice.to_vec()
    }
}
