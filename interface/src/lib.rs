//! Interface crate for Lazorkit V2.
//!
//! This crate provides client-side interfaces and utilities for interacting
//! with the Lazorkit V2 program.

pub use lazorkit_v2_state::plugin::PluginEntry;

/// Plugin instruction types
#[repr(u8)]
pub enum PluginInstruction {
    CheckPermission = 0,
    InitConfig = 1,
    UpdateConfig = 2,
}

/// Arguments for CheckPermission instruction
#[derive(Debug)]
pub struct CheckPermissionArgs {
    pub instruction_data_len: u16,
}
