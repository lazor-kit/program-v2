pub mod authority;
pub mod session;
pub mod wallet;

#[repr(u8)]
pub enum AccountDiscriminator {
    Wallet = 1,
    Authority = 2,
    Session = 3,
}
