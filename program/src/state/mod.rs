pub mod action;
pub mod authority;
pub mod deferred;
pub mod session;
pub mod wallet;

/// Discriminators for account types to ensure type safety.
#[repr(u8)]
pub enum AccountDiscriminator {
    /// The main Wallet account (Trust Anchor).
    Wallet = 1,
    /// An Authority account (Owner/Admin/Spender).
    Authority = 2,
    /// A Session account (Ephemeral Spender).
    Session = 3,
    /// A Deferred Execution authorization account.
    DeferredExec = 4,
}

/// Helper constant for versioning.
///
/// Current account logic version.
pub const CURRENT_ACCOUNT_VERSION: u8 = 1;
