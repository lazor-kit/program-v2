use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

#[repr(C, align(8))]
#[derive(NoPadding)]
/// Ephemeral Session Account.
///
/// Represents a temporary delegated authority with an expiration time.
#[repr(C, align(8))]
#[derive(NoPadding)]
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
