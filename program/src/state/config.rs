use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

#[repr(C, align(8))]
#[derive(NoPadding, Debug, Clone, Copy)]
/// Global Configuration Account for LazorKit.
///
/// Stores protocol-wide settings such as the admin key, and fee structures.
/// There is only expected to be a single PDA of this type at seeds `["config"]`.
pub struct ConfigAccount {
    /// Account discriminator (must be `4` for Config).
    pub discriminator: u8,
    /// Bump seed used to derive this PDA.
    pub bump: u8,
    /// Account Version (for future upgrades).
    pub version: u8,
    /// Number of treasury shards to distribute fees across. Max 255.
    pub num_shards: u8,
    /// Padding for 8-byte alignment.
    pub _padding: [u8; 4],
    /// The public key of the contract administrator.
    pub admin: Pubkey,
    /// The fixed fee (in lamports) charged for creating a new wallet.
    pub wallet_fee: u64,
    /// The fee (in lamports) charged for all other protocol actions.
    pub action_fee: u64,
}
// Size: 1 + 1 + 1 + 1 + 4 + 32 + 8 + 8 = 56 bytes.
