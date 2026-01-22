use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

#[repr(C, align(8))]
#[derive(NoPadding, Debug, Clone, Copy)]
pub struct AuthorityAccountHeader {
    pub discriminator: u8,
    pub authority_type: u8,
    pub role: u8,
    pub bump: u8,
    pub wallet: Pubkey,
    pub _padding: [u8; 4], // Align 36 -> 40
}
// 36 + 4 = 40. 40 is divisible by 8.
