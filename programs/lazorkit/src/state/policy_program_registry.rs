use anchor_lang::prelude::*;

/// Registry of approved policy programs that can govern smart wallet operations
#[account]
#[derive(Debug, InitSpace)]
pub struct PolicyProgramRegistry {
    /// List of registered policy program addresses
    #[max_len(10)]
    pub registered_programs: Vec<Pubkey>,
    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl PolicyProgramRegistry {
    pub const PREFIX_SEED: &'static [u8] = b"policy_registry";
}
