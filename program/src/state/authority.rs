use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

/// Header for all Authority accounts.
///
/// This header is followed by variable-length data depending on the `authority_type`.
#[repr(C, align(8))]
#[derive(NoPadding, Debug, Clone, Copy)]
pub struct AuthorityAccountHeader {
    /// Account discriminator (must be `2` for Authority).
    pub discriminator: u8,
    /// Type of authority: `0` = Ed25519, `1` = Secp256r1 (WebAuthn).
    pub authority_type: u8,
    /// Permission role: `0` = Owner, `1` = Admin, `2` = Spender.
    pub role: u8,
    /// Bump seed used to derive this PDA.
    pub bump: u8,
    /// Account Version (for future upgrades).
    pub version: u8,
    /// Padding for 8-byte alignment.
    pub _padding: [u8; 3],
    /// Monotonically increasing counter to prevent replay attacks (Secp256r1 only).
    pub counter: u64,
    /// The wallet this authority belongs to.
    pub wallet: Pubkey,
}
// 4 + 4 + 8 + 32 = 48. 48 is divisible by 8.
