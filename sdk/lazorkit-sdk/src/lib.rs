pub mod advanced;
pub mod basic;
pub mod core;
pub mod error;
pub mod types;
pub mod utils;

pub use crate::core::connection::SolConnection;
pub use crate::core::signer::LazorSigner;
pub use crate::error::{LazorSdkError, Result};
pub use crate::types::{RoleInfo, WalletInfo};
pub use crate::utils::{
    derive_config_pda, derive_vault_pda, fetch_wallet_account, fetch_wallet_info,
    find_role, parse_roles, parse_wallet_header,
};

pub mod state {
    pub use lazorkit_state::authority::AuthorityType;
    pub use lazorkit_state::{IntoBytes, LazorKitWallet, Position};
}
