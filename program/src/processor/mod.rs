//! Processor modules for handling program instructions.
//!
//! Each module corresponds to a specific instruction in the IDL.

pub mod close_session;
pub mod close_wallet;
pub mod create_session;
pub mod create_wallet;
pub mod execute;
pub mod init_treasury_shard;
pub mod initialize_config;
pub mod manage_authority;
pub mod sweep_treasury;
pub mod transfer_ownership;
pub mod update_config;
