use core::ops::Deref;
use pinocchio::{
    account_info::{AccountInfo, Ref},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::error::AuthError;

// SysvarS1otHashes111111111111111111111111111
pub const SLOT_HASHES_ID: Pubkey = [
    0x06, 0xa7, 0xd5, 0x17, 0x19, 0x2f, 0x0a, 0xaf, 0xc6, 0xf2, 0x65, 0xe3, 0xfb, 0x77, 0xcc, 0x7a,
    0xda, 0x82, 0xc5, 0x29, 0xd0, 0xbe, 0x3b, 0x13, 0x6e, 0x2d, 0x00, 0x55, 0x20, 0x00, 0x00, 0x00,
];

#[repr(C)]
pub struct SlotHash {
    pub height: u64,
    pub hash: [u8; 32],
}

pub struct SlotHashes<T>
where
    T: Deref<Target = [u8]>,
{
    data: T,
}

impl<'a, T> SlotHashes<T>
where
    T: Deref<Target = [u8]>,
{
    /// Creates a new `SlotHashes` struct.
    /// `data` is the slot hashes sysvar account data.
    #[inline(always)]
    pub unsafe fn new_unchecked(data: T) -> Self {
        SlotHashes { data }
    }

    /// Returns the number of slot hashes in the SlotHashes sysvar.
    #[inline(always)]
    pub fn get_slothashes_len(&self) -> u64 {
        let raw_ptr = self.data.as_ptr() as *const u8;
        unsafe { u64::from_le(*(raw_ptr as *const u64)) }
    }

    /// Returns the slot hash at the specified index.
    #[inline(always)]
    pub unsafe fn get_slot_hash_unchecked(&self, index: usize) -> &SlotHash {
        let offset = self
            .data
            .as_ptr()
            .add(8 + index * core::mem::size_of::<SlotHash>());
        &*(offset as *const SlotHash)
    }

    /// Returns the slot hash at the specified index.
    #[inline(always)]
    pub fn get_slot_hash(&self, index: usize) -> Result<&SlotHash, ProgramError> {
        if index > self.get_slothashes_len() as usize {
            return Err(AuthError::PermissionDenied.into()); // Mapping generic error for simplicity
        }
        unsafe { Ok(self.get_slot_hash_unchecked(index)) }
    }
}

impl<'a> TryFrom<&'a AccountInfo> for SlotHashes<Ref<'a, [u8]>> {
    type Error = ProgramError;

    #[inline(always)]
    fn try_from(account_info: &'a AccountInfo) -> Result<Self, Self::Error> {
        if account_info.key() != &SLOT_HASHES_ID {
            return Err(ProgramError::UnsupportedSysvar);
        }

        Ok(SlotHashes {
            data: account_info.try_borrow_data()?,
        })
    }
}
