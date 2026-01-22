use no_padding::NoPadding;

#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct WalletAccount {
    pub discriminator: u8,
    pub bump: u8,
    pub _padding: [u8; 6],
}
