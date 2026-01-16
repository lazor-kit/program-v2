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
    msg!("SolLimit: processing verification");

    if instruction_data.len() < VerifyInstruction::LEN {
        msg!("SolLimit: instruction data too short");
        return Err(ProgramError::InvalidInstructionData);
    }

    // Cast instruction data to VerifyInstruction
    let verify_ix = unsafe { &*(instruction_data.as_ptr() as *const VerifyInstruction) };

    if verify_ix.discriminator != INSTRUCTION_VERIFY {
        msg!("SolLimit: invalid instruction discriminator");
        return Err(ProgramError::InvalidInstructionData);
    }

    // accounts[0] is the LazorKit wallet config account
    let config_account = &accounts[0];
    msg!("SolLimit: config account: {:?}", config_account.key());

    let state_offset = verify_ix.state_offset as usize;
    msg!("SolLimit: state offset: {}", state_offset);

    // Safety check for offset
    let config_data_len = config_account.data_len();
    msg!("SolLimit: config data len: {}", config_data_len);
    if state_offset + SolLimitState::LEN > config_data_len {
        msg!(
            "SolLimit: state offset out of bounds. Offset + Len: {}",
            state_offset + SolLimitState::LEN
        );
        return Err(ProgramError::InvalidAccountData);
    }

    // Read state (read-only)
    let config_data = unsafe { config_account.borrow_data_unchecked() };
    let state_ptr = unsafe { config_data[state_offset..].as_ptr() as *const SolLimitState };
    let mut state = unsafe { *state_ptr };

    msg!(
        "SolLimit: Current amount: {}, Spending: {}",
        state.amount,
        verify_ix.amount
    );

    if state.amount < verify_ix.amount {
        msg!(
            "SolLimit: Insufficient SOL limit. Needed: {}, Rem: {}",
            verify_ix.amount,
            state.amount
        );
        return Err(lazorkit_interface::PluginError::VerificationFailed.into());
    }

    state.amount -= verify_ix.amount;
    msg!("SolLimit: New amount: {}", state.amount);

    // Set return data
    unsafe {
        sol_set_return_data(
            &state as *const SolLimitState as *const u8,
            SolLimitState::LEN as u64,
        );
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
