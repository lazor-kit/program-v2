use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

/// Size of the fixed session header (excluding actions).
pub const SESSION_HEADER_SIZE: usize = 80;

#[repr(C, align(8))]
#[derive(NoPadding)]
/// Ephemeral Session Account.
///
/// Represents a temporary delegated authority with an expiration time.
/// Optional actions may follow the 80-byte header as a flat byte buffer.
pub struct SessionAccount {
    /// Account discriminator (must be `3` for Session).
    pub discriminator: u8, // 1
    /// Bump seed for this PDA.
    pub bump: u8, // 1
    /// Account Version.
    pub version: u8, // 1
    /// Padding for alignment.
    pub _padding: [u8; 5], // 5
    /// The wallet this session belongs to.
    pub wallet: Pubkey, // 32
    /// The ephemeral public key authorized to sign.
    pub session_key: Pubkey, // 32
    /// Absolute slot height when this session expires.
    pub expires_at: u64, // 8
}

/// Returns true if the session account data contains actions after the header.
#[inline]
pub fn has_actions(session_data: &[u8]) -> bool {
    session_data.len() > SESSION_HEADER_SIZE
}

/// Returns the actions buffer slice (bytes after the 80-byte header).
/// Returns empty slice if no actions.
#[inline]
pub fn actions_slice(session_data: &[u8]) -> &[u8] {
    if session_data.len() > SESSION_HEADER_SIZE {
        &session_data[SESSION_HEADER_SIZE..]
    } else {
        &[]
    }
}
