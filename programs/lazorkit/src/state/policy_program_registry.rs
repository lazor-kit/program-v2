use anchor_lang::prelude::*;

/// Registry of approved policy programs for smart wallet operations
///
/// Maintains a whitelist of policy programs that can be used to govern
/// smart wallet transaction validation and security rules.
#[account]
#[derive(Debug, InitSpace)]
pub struct PolicyProgramRegistry {
    /// List of registered policy program addresses (max 10)
    #[max_len(10)]
    pub registered_programs: Vec<Pubkey>,
    /// Bump seed for PDA derivation and verification
    pub bump: u8,
}

impl PolicyProgramRegistry {
    /// Seed prefix used for PDA derivation of the policy registry account
    pub const PREFIX_SEED: &'static [u8] = b"policy_registry";
}
