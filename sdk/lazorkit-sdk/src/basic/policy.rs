use crate::state::{IntoBytes, PolicyHeader};
use solana_sdk::pubkey::Pubkey;

/// Fluent builder for constructing the `policies_config` byte buffer.
/// This buffer is passed to the `AddAuthority` and `UpdateAuthority` instructions.
#[derive(Default)]
pub struct PolicyConfigBuilder {
    policies: Vec<(Pubkey, Vec<u8>)>,
}

impl PolicyConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a policy to the configuration.
    /// `program_id`: The address of the policy program.
    /// `state`: The policy-specific state blob (e.g. `SolLimitState`).
    pub fn add_policy(mut self, program_id: Pubkey, state: Vec<u8>) -> Self {
        self.policies.push((program_id, state));
        self
    }

    /// Build the serialized byte buffer.
    pub fn build(self) -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut current_offset = 0;

        for (pid, state) in self.policies {
            let state_len = state.len();
            let boundary = current_offset + PolicyHeader::LEN + state_len;

            let header = PolicyHeader {
                program_id: pid.to_bytes(),
                data_length: state_len as u16,
                _padding: 0,
                boundary: boundary as u32,
            };

            if let Ok(header_bytes) = header.into_bytes() {
                buffer.extend_from_slice(header_bytes);
                buffer.extend_from_slice(&state);
                current_offset = boundary;
            }
        }

        buffer
    }
}
