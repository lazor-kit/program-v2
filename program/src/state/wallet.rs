use no_padding::NoPadding;

// Main Wallet Account.
// Acts as the trust anchor. Assets are stored in the separate Vault PDA.
#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct WalletAccount {
    /// Account discriminator (must be `1` for Wallet).
    pub discriminator: u8,
    /// Bump seed for this PDA.
    pub bump: u8,
    /// Account Version.
    pub version: u8,
    /// Padding for alignment.
    pub _padding: [u8; 5],
}
