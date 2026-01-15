use pinocchio::program_error::ProgramError;

/// Trait for types that can be transmuted from bytes
pub trait Transmutable: Sized {
    const LEN: usize;

    /// Load from bytes without copying (unsafe)
    unsafe fn load_unchecked(data: &[u8]) -> Result<&Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(&*(data.as_ptr() as *const Self))
    }
}

/// Trait for mutable transmutation
pub trait TransmutableMut: Transmutable {
    /// Load mutable reference from bytes (unsafe)
    unsafe fn load_mut_unchecked(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(&mut *(data.as_mut_ptr() as *mut Self))
    }
}

/// Trait for types that can be converted into bytes reference
pub trait IntoBytes {
    fn into_bytes(&self) -> Result<&[u8], ProgramError>;
}
