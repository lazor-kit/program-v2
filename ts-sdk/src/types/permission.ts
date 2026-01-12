/**
 * Role Permission enum for inline permission checking
 * 
 * These permissions are checked directly in the Lazorkit V2 contract,
 * without requiring CPI to external plugins.
 */
export enum RolePermission {
  /** All permissions - Can execute any instruction */
  All = 0,
  
  /** Manage Authority only - Can only add/remove/update authorities */
  /** Cannot execute regular transactions */
  ManageAuthority = 1,
  
  /** All but Manage Authority - Can execute any instruction except authority management */
  AllButManageAuthority = 2,
  
  /** Execute Only - Can only execute transactions, cannot manage authorities or plugins */
  /** Most restrictive permission level */
  ExecuteOnly = 3,
}

/**
 * Check if a role permission allows executing a regular instruction
 */
export function allowsExecute(permission: RolePermission): boolean {
  return [
    RolePermission.All,
    RolePermission.AllButManageAuthority,
    RolePermission.ExecuteOnly,
  ].includes(permission);
}

/**
 * Check if a role permission allows managing authorities
 */
export function allowsManageAuthority(permission: RolePermission): boolean {
  return [
    RolePermission.All,
    RolePermission.ManageAuthority,
  ].includes(permission);
}

/**
 * Check if a role permission allows managing plugins
 */
export function allowsManagePlugin(permission: RolePermission): boolean {
  return permission === RolePermission.All;
}

/**
 * Get default role permission for new authorities
 */
export function getDefaultRolePermission(): RolePermission {
  return RolePermission.AllButManageAuthority;
}
