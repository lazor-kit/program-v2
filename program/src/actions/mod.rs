//! Actions module - Individual instruction handlers
//!
//! Each instruction has its own file for better maintainability.

pub mod add_authority;
pub mod create_session;
pub mod create_wallet;
pub mod execute;
pub mod remove_authority;
pub mod transfer_ownership;
pub mod update_authority;

// Re-export all processors
pub use add_authority::process_add_authority;
pub use create_session::process_create_session;
pub use create_wallet::process_create_wallet;
pub use execute::process_execute;
pub use remove_authority::process_remove_authority;
pub use transfer_ownership::process_transfer_ownership;
pub use update_authority::process_update_authority;
