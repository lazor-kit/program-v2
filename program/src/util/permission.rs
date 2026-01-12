//! Permission checking utilities for Hybrid Architecture
//!
//! This module provides inline permission checking for role permissions,
//! which are checked directly in the Lazorkit V2 contract without CPI.
//! Other permissions (SolLimit, TokenLimit, etc.) are handled by external plugins.

use crate::error::LazorkitError;
use lazorkit_v2_state::{role_permission::RolePermission, wallet_account::AuthorityData};
use pinocchio::{
    account_info::AccountInfo, instruction::Instruction, program_error::ProgramError, ProgramResult,
};

/// Check inline role permission for an instruction
///
/// This is the first check in the Hybrid architecture:
/// 1. Check inline role permission (here)
/// 2. If allowed, check external plugins (via CPI)
///
/// # Arguments
/// * `role_permission` - The role permission to check
/// * `instruction` - The instruction to check permission for
/// * `is_authority_management` - Whether this is an authority management instruction
///
/// # Returns
/// * `Ok(())` - If permission is granted
/// * `Err(ProgramError)` - If permission is denied
pub fn check_role_permission(
    role_permission: RolePermission,
    is_authority_management: bool,
) -> ProgramResult {
    match role_permission {
        RolePermission::All => {
            // All permissions - allow everything
            Ok(())
        }
        RolePermission::ManageAuthority => {
            // Only allow authority management
            if is_authority_management {
                Ok(())
            } else {
                Err(LazorkitError::PermissionDenied.into())
            }
        }
        RolePermission::AllButManageAuthority => {
            // Allow everything except authority management
            if is_authority_management {
                Err(LazorkitError::PermissionDenied.into())
            } else {
                Ok(())
            }
        }
        RolePermission::ExecuteOnly => {
            // Only allow regular transactions
            if is_authority_management {
                Err(LazorkitError::PermissionDenied.into())
            } else {
                Ok(())
            }
        }
    }
}

/// Check if an instruction is an authority management instruction
///
/// Authority management instructions are:
/// - AddAuthority
/// - RemoveAuthority
/// - UpdateAuthority
/// - AddPlugin
/// - RemovePlugin
/// - UpdatePlugin
pub fn is_authority_management_instruction(instruction: &Instruction) -> bool {
    // Check if instruction is from Lazorkit V2 program
    // For now, we assume all instructions from other programs are regular transactions
    // Authority management instructions are handled at the action level, not instruction level

    // This function is used for checking embedded instructions in sign()
    // Authority management instructions are separate actions, not embedded instructions
    false
}

/// Check inline role permission for authority management action
///
/// This is used for add_authority, remove_authority, update_authority, etc.
pub fn check_role_permission_for_authority_management(
    authority_data: &AuthorityData,
) -> ProgramResult {
    let role_permission = authority_data
        .position
        .role_permission()
        .map_err(|_| LazorkitError::InvalidRolePermission)?;

    // Authority management actions require ManageAuthority or All permission
    if role_permission.allows_manage_authority() {
        Ok(())
    } else {
        Err(LazorkitError::PermissionDenied.into())
    }
}

/// Check inline role permission for plugin management action
///
/// This is used for add_plugin, remove_plugin, update_plugin
pub fn check_role_permission_for_plugin_management(
    authority_data: &AuthorityData,
) -> ProgramResult {
    let role_permission = authority_data
        .position
        .role_permission()
        .map_err(|_| LazorkitError::InvalidRolePermission)?;

    // Plugin management requires All permission
    if role_permission.allows_manage_plugin() {
        Ok(())
    } else {
        Err(LazorkitError::PermissionDenied.into())
    }
}

/// Check inline role permission for regular transaction execution
///
/// This is used for sign() instruction
/// Returns (has_all_permission, should_skip_plugin_checks)
/// - has_all_permission: true if All permission (can skip all checks)
/// - should_skip_plugin_checks: true if All or AllButManageAuthority (skip plugin checks for regular transactions)
pub fn check_role_permission_for_execute(
    authority_data: &AuthorityData,
) -> Result<(bool, bool), ProgramError> {
    let role_permission = authority_data
        .position
        .role_permission()
        .map_err(|_| LazorkitError::InvalidRolePermission)?;

    match role_permission {
        RolePermission::All => {
            // All permission - can skip all plugin checks
            Ok((true, true))
        }
        RolePermission::AllButManageAuthority => {
            // AllButManageAuthority - can skip plugin checks for regular transactions
            // (but still need to check for authority management instructions)
            Ok((false, true))
        }
        RolePermission::ExecuteOnly => {
            // ExecuteOnly - need to check plugins
            Ok((false, false))
        }
        RolePermission::ManageAuthority => {
            // ManageAuthority - cannot execute regular transactions
            Err(LazorkitError::PermissionDenied.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazorkit_v2_state::{
        role_permission::RolePermission,
        wallet_account::AuthorityData,
        position::Position,
        authority::AuthorityType,
    };

    #[test]
    fn test_check_role_permission_all() {
        // Test All permission allows everything
        assert!(check_role_permission(RolePermission::All, false).is_ok());
        assert!(check_role_permission(RolePermission::All, true).is_ok());
    }

    #[test]
    fn test_check_role_permission_manage_authority() {
        // Test ManageAuthority only allows authority management
        assert!(check_role_permission(RolePermission::ManageAuthority, true).is_ok());
        assert!(check_role_permission(RolePermission::ManageAuthority, false).is_err());
    }

    #[test]
    fn test_check_role_permission_all_but_manage_authority() {
        // Test AllButManageAuthority allows everything except authority management
        assert!(check_role_permission(RolePermission::AllButManageAuthority, false).is_ok());
        assert!(check_role_permission(RolePermission::AllButManageAuthority, true).is_err());
    }

    #[test]
    fn test_check_role_permission_execute_only() {
        // Test ExecuteOnly only allows regular transactions
        assert!(check_role_permission(RolePermission::ExecuteOnly, false).is_ok());
        assert!(check_role_permission(RolePermission::ExecuteOnly, true).is_err());
    }

    fn create_mock_authority_data(role_permission: RolePermission) -> AuthorityData {
        use lazorkit_v2_state::plugin_ref::PluginRef;
        let position = Position::new(
            1, // authority_type: Ed25519
            64, // authority_length
            0, // num_plugin_refs
            role_permission,
            1, // id
            100, // boundary
        );
        AuthorityData {
            position,
            authority_data: vec![0u8; 64],
            plugin_refs: vec![],
        }
    }

    #[test]
    fn test_check_role_permission_for_authority_management_all() {
        // Test All permission allows authority management
        let authority_data = create_mock_authority_data(RolePermission::All);
        assert!(check_role_permission_for_authority_management(&authority_data).is_ok());
    }

    #[test]
    fn test_check_role_permission_for_authority_management_manage_authority() {
        // Test ManageAuthority permission allows authority management
        let authority_data = create_mock_authority_data(RolePermission::ManageAuthority);
        assert!(check_role_permission_for_authority_management(&authority_data).is_ok());
    }

    #[test]
    fn test_check_role_permission_for_authority_management_execute_only() {
        // Test ExecuteOnly permission denies authority management
        let authority_data = create_mock_authority_data(RolePermission::ExecuteOnly);
        assert!(check_role_permission_for_authority_management(&authority_data).is_err());
    }

    #[test]
    fn test_check_role_permission_for_plugin_management_all() {
        // Test All permission allows plugin management
        let authority_data = create_mock_authority_data(RolePermission::All);
        assert!(check_role_permission_for_plugin_management(&authority_data).is_ok());
    }

    #[test]
    fn test_check_role_permission_for_plugin_management_manage_authority() {
        // Test ManageAuthority permission denies plugin management
        let authority_data = create_mock_authority_data(RolePermission::ManageAuthority);
        assert!(check_role_permission_for_plugin_management(&authority_data).is_err());
    }

    #[test]
    fn test_check_role_permission_for_execute_all() {
        // Test All permission returns (true, true) - skip all checks
        let authority_data = create_mock_authority_data(RolePermission::All);
        let result = check_role_permission_for_execute(&authority_data).unwrap();
        assert_eq!(result, (true, true));
    }

    #[test]
    fn test_check_role_permission_for_execute_all_but_manage_authority() {
        // Test AllButManageAuthority returns (false, true) - skip plugin checks
        let authority_data = create_mock_authority_data(RolePermission::AllButManageAuthority);
        let result = check_role_permission_for_execute(&authority_data).unwrap();
        assert_eq!(result, (false, true));
    }

    #[test]
    fn test_check_role_permission_for_execute_execute_only() {
        // Test ExecuteOnly returns (false, false) - check plugins
        let authority_data = create_mock_authority_data(RolePermission::ExecuteOnly);
        let result = check_role_permission_for_execute(&authority_data).unwrap();
        assert_eq!(result, (false, false));
    }

    #[test]
    fn test_check_role_permission_for_execute_manage_authority() {
        // Test ManageAuthority returns error - cannot execute
        let authority_data = create_mock_authority_data(RolePermission::ManageAuthority);
        assert!(check_role_permission_for_execute(&authority_data).is_err());
    }
}
