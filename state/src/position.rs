//! Position structure - For plugin references

use crate::authority::AuthorityType;
use crate::role_permission::RolePermission;
use crate::{IntoBytes, Transmutable, TransmutableMut};
use no_padding::NoPadding;
use pinocchio::program_error::ProgramError;

/// Position structure - Defines authority structure
///
/// Position structure, uses num_plugin_refs instead of num_actions
/// and includes inline role_permission for Hybrid architecture
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, NoPadding)]
pub struct Position {
    /// Authority type (Ed25519, Secp256r1, etc.)
    pub authority_type: u16, // 2 bytes
    /// Length of authority data
    pub authority_length: u16, // 2 bytes
    /// Number of plugin references (thay vì num_actions)
    pub num_plugin_refs: u16, // 2 bytes
    /// Inline role permission (Hybrid architecture)
    pub role_permission: u8, // 1 byte (RolePermission enum)
    padding: u8, // 1 byte
    /// Unique authority ID
    pub id: u32, // 4 bytes
    /// Boundary marker (end of this authority data)
    pub boundary: u32, // 4 bytes
}

impl Position {
    pub const LEN: usize = core::mem::size_of::<Self>();

    /// Create a new Position
    pub fn new(
        authority_type: u16,
        authority_length: u16,
        num_plugin_refs: u16,
        role_permission: RolePermission,
        id: u32,
        boundary: u32,
    ) -> Self {
        Self {
            authority_type,
            authority_length,
            num_plugin_refs,
            role_permission: role_permission as u8,
            padding: 0,
            id,
            boundary,
        }
    }

    /// Get role permission
    pub fn role_permission(&self) -> Result<RolePermission, ProgramError> {
        RolePermission::try_from(self.role_permission)
    }

    /// Get authority type
    pub fn authority_type(&self) -> Result<AuthorityType, ProgramError> {
        AuthorityType::try_from(self.authority_type)
    }

    /// Get authority ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get authority length
    pub fn authority_length(&self) -> u16 {
        self.authority_length
    }

    /// Get number of plugin references
    pub fn num_plugin_refs(&self) -> u16 {
        self.num_plugin_refs
    }

    /// Get boundary
    pub fn boundary(&self) -> u32 {
        self.boundary
    }
}

impl Transmutable for Position {
    const LEN: usize = Self::LEN;
}

impl TransmutableMut for Position {}

impl IntoBytes for Position {
    fn into_bytes(&self) -> Result<&[u8], ProgramError> {
        let bytes =
            unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN) };
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_creation() {
        use crate::role_permission::RolePermission;
        let pos = Position::new(1, 64, 2, RolePermission::All, 100, 200);
        assert_eq!(pos.authority_type, 1);
        assert_eq!(pos.authority_length, 64);
        assert_eq!(pos.num_plugin_refs, 2);
        assert_eq!(pos.role_permission, RolePermission::All as u8);
        assert_eq!(pos.id, 100);
        assert_eq!(pos.boundary, 200);
    }

    #[test]
    fn test_position_size() {
        assert_eq!(Position::LEN, 16);
    }

    #[test]
    fn test_position_serialization() {
        use crate::role_permission::RolePermission;
        let pos = Position::new(1, 64, 2, RolePermission::AllButManageAuthority, 100, 200);
        let bytes = pos.into_bytes().unwrap();
        assert_eq!(bytes.len(), Position::LEN);

        // Deserialize
        let loaded = unsafe { Position::load_unchecked(bytes).unwrap() };
        assert_eq!(*loaded, pos);
    }
}
