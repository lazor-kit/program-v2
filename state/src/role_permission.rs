//! Role Permission enum for inline permission checking (Hybrid Architecture)
//!
//! This module defines the inline role permissions that are checked directly
//! in the Lazorkit V2 contract, similar to inline actions.
//! Other permissions (SolLimit, TokenLimit, ProgramWhitelist, etc.) are
//! handled by external plugins.

use pinocchio::program_error::ProgramError;

/// Role Permission - Inline permission types (Hybrid Architecture)
///
/// These permissions are checked directly in the Lazorkit V2 contract,
/// without requiring CPI to external plugins. This provides:
/// - Faster execution (no CPI overhead for common checks)
/// - Simpler UX for basic use cases
/// - Better security for core wallet operations
///
/// Other permissions (SolLimit, TokenLimit, ProgramWhitelist, etc.)
/// are handled by external plugins for flexibility.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolePermission {
    /// All permissions - Can execute any instruction
    /// Similar to `All` action
    All = 0,

    /// Manage Authority only - Can only add/remove/update authorities
    /// Cannot execute regular transactions
    /// Similar to `ManageAuthority` action
    ManageAuthority = 1,

    /// All but Manage Authority - Can execute any instruction except authority management
    /// Similar to `AllButManageAuthority` action
    AllButManageAuthority = 2,

    /// Execute Only - Can only execute transactions, cannot manage authorities or plugins
    /// Most restrictive permission level
    ExecuteOnly = 3,
}

impl TryFrom<u8> for RolePermission {
    type Error = ProgramError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RolePermission::All),
            1 => Ok(RolePermission::ManageAuthority),
            2 => Ok(RolePermission::AllButManageAuthority),
            3 => Ok(RolePermission::ExecuteOnly),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

impl From<RolePermission> for u8 {
    fn from(role: RolePermission) -> Self {
        role as u8
    }
}

impl RolePermission {
    /// Check if this role permission allows executing a regular instruction
    pub fn allows_execute(&self) -> bool {
        matches!(
            self,
            RolePermission::All
                | RolePermission::AllButManageAuthority
                | RolePermission::ExecuteOnly
        )
    }

    /// Check if this role permission allows managing authorities
    pub fn allows_manage_authority(&self) -> bool {
        matches!(self, RolePermission::All | RolePermission::ManageAuthority)
    }

    /// Check if this role permission allows managing plugins
    pub fn allows_manage_plugin(&self) -> bool {
        matches!(self, RolePermission::All)
    }

    /// Get default role permission for new authorities
    pub fn default() -> Self {
        RolePermission::AllButManageAuthority
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_permission_from_u8() {
        assert_eq!(RolePermission::try_from(0).unwrap(), RolePermission::All);
        assert_eq!(
            RolePermission::try_from(1).unwrap(),
            RolePermission::ManageAuthority
        );
        assert_eq!(
            RolePermission::try_from(2).unwrap(),
            RolePermission::AllButManageAuthority
        );
        assert_eq!(
            RolePermission::try_from(3).unwrap(),
            RolePermission::ExecuteOnly
        );
        assert!(RolePermission::try_from(4).is_err());
    }

    #[test]
    fn test_role_permission_to_u8() {
        assert_eq!(u8::from(RolePermission::All), 0);
        assert_eq!(u8::from(RolePermission::ManageAuthority), 1);
        assert_eq!(u8::from(RolePermission::AllButManageAuthority), 2);
        assert_eq!(u8::from(RolePermission::ExecuteOnly), 3);
    }

    #[test]
    fn test_allows_execute() {
        assert!(RolePermission::All.allows_execute());
        assert!(!RolePermission::ManageAuthority.allows_execute());
        assert!(RolePermission::AllButManageAuthority.allows_execute());
        assert!(RolePermission::ExecuteOnly.allows_execute());
    }

    #[test]
    fn test_allows_manage_authority() {
        assert!(RolePermission::All.allows_manage_authority());
        assert!(RolePermission::ManageAuthority.allows_manage_authority());
        assert!(!RolePermission::AllButManageAuthority.allows_manage_authority());
        assert!(!RolePermission::ExecuteOnly.allows_manage_authority());
    }

    #[test]
    fn test_allows_manage_plugin() {
        assert!(RolePermission::All.allows_manage_plugin());
        assert!(!RolePermission::ManageAuthority.allows_manage_plugin());
        assert!(!RolePermission::AllButManageAuthority.allows_manage_plugin());
        assert!(!RolePermission::ExecuteOnly.allows_manage_plugin());
    }
}
