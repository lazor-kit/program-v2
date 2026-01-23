use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

#[repr(C, align(8))]
#[derive(NoPadding, Debug, Clone, Copy)]
pub struct AuthorityAccountHeader {
    pub discriminator: u8,
    pub authority_type: u8,
    pub role: u8,
    pub bump: u8,
    pub _padding: [u8; 4],
    pub counter: u64,
    pub wallet: Pubkey,
}
// 4 + 4 + 8 + 32 = 48. 48 is divisible by 8.
