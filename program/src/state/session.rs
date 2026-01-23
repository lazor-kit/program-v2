use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct SessionAccount {
    pub discriminator: u8,   // 1
    pub bump: u8,            // 1
    pub _padding: [u8; 6],   // 6
    pub wallet: Pubkey,      // 32
    pub session_key: Pubkey, // 32
    pub expires_at: u64,     // 8
}
