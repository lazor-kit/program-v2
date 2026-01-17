use lazorkit_state::{policy::PolicyHeader, IntoBytes};
use no_padding::NoPadding;
use pinocchio::program_error::ProgramError;
use std::slice;

/// State for the SOL limit policy (Client-side definition)
/// Matches on-chain layout.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, NoPadding)]
pub struct SolLimitState {
    pub amount: u64,
}

impl SolLimitState {
    pub const LEN: usize = 8;
}

impl IntoBytes for SolLimitState {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        Ok(unsafe { slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) })
    }
}

pub struct SolLimitBuilder {
    amount: u64,
}

impl SolLimitBuilder {
    pub fn new() -> Self {
        Self { amount: 0 }
    }

    pub fn limit(mut self, lamports: u64) -> Self {
        self.amount = lamports;
        self
    }

    /// Build the raw state bytes
    pub fn build_state(self) -> Vec<u8> {
        let state = SolLimitState {
            amount: self.amount,
        };
        // We can't return slice refernece easily from local struct, so we copy to vec
        let slice = unsafe {
            slice::from_raw_parts(
                &state as *const SolLimitState as *const u8,
                SolLimitState::LEN,
            )
        };
        slice.to_vec()
    }

    /// Build the full Policy Blob (Header + State)
    pub fn build_blob(self, policy_program_id: [u8; 32]) -> Vec<u8> {
        let state_bytes = self.build_state();
        let header = PolicyHeader {
            program_id: policy_program_id,

            // For now, let's strictly return the state bytes. The Core SDK's `add_authority`
            // usually concatenates them. Or we return a tuple.
            // Let's return the state bytes for now.
            // The "Full Blob" implies we know the offset, which we don't until we know order.
            // Wait, SolLimit on-chain `process_instruction` checks `VerifyInstruction` from input data.
            // The Header stores `verifying_instruction_offset` etc?
            // No, Header stores `state_offset` implicitly by its position?
            // Let's check `PolicyHeader` definition in `lazorkit-state`.
            // The Header is [program_id(32), len(2), offset(2)].
            // We need to know where the state will be.
            // If this is the *first* policy, offset is Header::LEN.
            // This Builder might be too simple. It usually just returns the config bytes,
            // and the `InstructionBuilder` in Core wraps them and calculates offsets.
            // OR the user manually builds it.

            // For now, let's strictly return the state bytes. The Core SDK's `add_authority`
            // usually concatenates them. Or we return a tuple.
            // Let's return the state bytes for now.
            // The "Full Blob" implies we know the offset, which we don't until we know order.

            // But wait, PolicyHeader is: program_id, data_len, state_offset.
            // We know data_len (8). We don't know state_offset.
            // So we can return a "UnplacedPolicy" struct?

            // Let's stick to returning types the user can feed into the generic builder.
            // Header size is 40. State size is SolLimitState::LEN (8).
            // Boundary is current_offset (0 relative to this blob) + 40 + 8 = 48
            // But boundary is usually absolute offset within the policies buffer?
            // In the core contract `parse_policies`, boundary is used as `self.cursor = header.boundary`.
            // So yes, it is relative to the start of the policies config buffer.
            // Since we are returning a single blob here, we assume it starts at 0?
            // NO, the SDK user (AddAuthorityBuilder) appends this.
            // The Builder there must fix up the boundaries!

            // For this specific helper, we can't know the final boundary if we don't know where it's inserted.
            // Standard LazorKit pattern: The user construction might just return the state part?
            // OR we provide a "default" boundary that needs patching.

            // However, the error was just "no field named state_offset".
            // Let's fix fields first.
            _padding: 0,
            boundary: (PolicyHeader::LEN + SolLimitState::LEN) as u32, // Self-contained length
            data_length: SolLimitState::LEN as u16,
        };

        let mut blob = Vec::new();
        blob.extend_from_slice(header.into_bytes().unwrap());
        blob.extend_from_slice(&state_bytes);
        blob
    }
}
