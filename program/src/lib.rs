#![allow(unexpected_cfgs)]
pub mod auth;
pub mod compact;
pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;
pub mod utils;

use pinocchio_pubkey::declare_id;
declare_id!("2r5xXopRxWYcKHVrrzGrwfRJb3N2DSBkMgG93k6Z8ZFC");
