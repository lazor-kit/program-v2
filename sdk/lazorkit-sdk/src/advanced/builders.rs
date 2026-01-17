use lazorkit_state::{policy::PolicyHeader, IntoBytes};

/// Builder for constructing Policy configurations manually.
pub struct PolicyConfigBuilder {
    buffer: Vec<u8>,
}

impl PolicyConfigBuilder {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn add_policy(mut self, header: PolicyHeader, data: &[u8]) -> Self {
        // Enforce alignment or specific layout expectations here if needed
        // For now, simple append: [Header][Data]
        self.buffer.extend_from_slice(header.into_bytes().unwrap());
        self.buffer.extend_from_slice(data);
        self
    }

    pub fn build(self) -> Vec<u8> {
        self.buffer
    }
}
