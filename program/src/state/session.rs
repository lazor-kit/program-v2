use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct SessionAccount {
    pub discriminator: u64,  // 8
    pub wallet: Pubkey,      // 32
    pub session_key: Pubkey, // 32
    pub expires_at: i64,     // 8
    pub bump: u8,            // 1
    pub _padding: [u8; 7],   // 7
}
