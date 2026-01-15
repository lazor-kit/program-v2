use lazorkit_interface::{VerifyInstruction, INSTRUCTION_VERIFY};
use lazorkit_state::{IntoBytes, Transmutable, TransmutableMut};
use no_padding::NoPadding;
use pinocchio::{
    account_info::AccountInfo, entrypoint, msg, program_error::ProgramError, pubkey::Pubkey,
};

/// State for the SOL limit plugin.
///
/// This struct tracks and enforces a maximum amount of SOL that can be
/// used in operations. The limit is decreased as operations are performed.
///
/// This corresponds to the opaque state_blob stored after the PluginHeader.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, NoPadding)]
pub struct SolLimitState {
    /// The remaining amount of SOL that can be used (in lamports)
    pub amount: u64,
}

impl SolLimitState {
    /// Size of the SolLimitState struct in bytes
    pub const LEN: usize = 8;
}

impl TryFrom<&[u8]> for SolLimitState {
    type Error = ProgramError;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        // Safety: We checked length, and SolLimitState is Pod (NoPadding + repr(C))
        Ok(unsafe { *bytes.as_ptr().cast::<SolLimitState>() })
    }
}

impl Transmutable for SolLimitState {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for SolLimitState {}

impl IntoBytes for SolLimitState {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        Ok(unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) })
    }
}

#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    // 1. Parse instruction data (Zero-Copy)
    if instruction_data.len() < VerifyInstruction::LEN {
        msg!("Instruction data too short: {}", instruction_data.len());
        return Err(ProgramError::InvalidInstructionData);
    }

    // Safety: VerifyInstruction is #[repr(C)] (Verify using pointer cast)
    // Note: In a production environment, ensure alignment or use read_unaligned
    let instruction = unsafe { &*(instruction_data.as_ptr() as *const VerifyInstruction) };

    // 2. Verify discriminator
    if instruction.discriminator != INSTRUCTION_VERIFY {
        msg!(
            "Invalid instruction discriminator: {:x}",
            instruction.discriminator
        );
        return Err(ProgramError::InvalidInstructionData);
    }

    // 3. Get the account containing the state (LazorKit wallet account)
    if accounts.is_empty() {
        msg!("No accounts provided");
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let wallet_account = &accounts[0];

    // 4. Load state from the specified offset (READ ONLY)
    let offset = instruction.state_offset as usize;
    let data = wallet_account.try_borrow_data()?;

    // Ensure we don't read past end of data
    if offset + SolLimitState::LEN > data.len() {
        msg!("Account data too small for state offset {}", offset);
        return Err(ProgramError::AccountDataTooSmall);
    }

    // Load state reference
    let state_ref =
        unsafe { SolLimitState::load_unchecked(&data[offset..offset + SolLimitState::LEN])? };

    // Create local copy to modify
    let mut state = unsafe { core::ptr::read(state_ref as *const SolLimitState) };

    // 5. Enforce logic
    if instruction.amount > state.amount {
        msg!(
            "SolLimit exceeded: remaining {}, requested {}",
            state.amount,
            instruction.amount
        );
        return Err(ProgramError::Custom(0x1001)); // Insufficient balance error
    }

    // 6. Update state locally
    state.amount = state.amount.saturating_sub(instruction.amount);

    msg!("SolLimit approved. New amount: {}", state.amount);

    // 7. Return new state via Return Data
    let state_bytes = unsafe {
        core::slice::from_raw_parts(
            &state as *const SolLimitState as *const u8,
            SolLimitState::LEN,
        )
    };

    unsafe {
        sol_set_return_data(state_bytes.as_ptr(), state_bytes.len() as u64);
    }

    Ok(())
}

#[cfg(target_os = "solana")]
extern "C" {
    fn sol_set_return_data(data: *const u8, length: u64);
}

#[cfg(not(target_os = "solana"))]
unsafe fn sol_set_return_data(_data: *const u8, _length: u64) {
    // No-op on host
}
