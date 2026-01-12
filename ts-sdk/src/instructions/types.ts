/**
 * Instruction discriminators for Lazorkit V2
 * 
 * These match the LazorkitInstruction enum in Rust (u16)
 */
export enum LazorkitInstruction {
  /** Creates a new Lazorkit wallet */
  CreateSmartWallet = 0,
  /** Signs and executes a transaction with plugin checks */
  Sign = 1,
  /** Adds a new authority to the wallet */
  AddAuthority = 2,
  /** Adds a plugin to the wallet's plugin registry */
  AddPlugin = 3,
  /** Removes a plugin from the wallet's plugin registry */
  RemovePlugin = 4,
  /** Updates a plugin in the wallet's plugin registry */
  UpdatePlugin = 5,
  /** Updates an existing authority in the wallet */
  UpdateAuthority = 6,
  /** Removes an authority from the wallet */
  RemoveAuthority = 7,
  /** Creates a new authentication session for a wallet authority */
  CreateSession = 8,
}

/**
 * CreateSmartWallet instruction arguments
 * 
 * Layout: id[32] + bump[1] + wallet_bump[1] + first_authority_type[2] + 
 *         first_authority_data_len[2] + num_plugin_refs[2] + role_permission[1] + padding[1] = 43 bytes
 */
export interface CreateSmartWalletArgs {
  /** Unique wallet identifier (32 bytes) */
  id: Uint8Array;
  /** PDA bump for wallet_account */
  bump: number;
  /** PDA bump for wallet_vault */
  walletBump: number;
  /** Type of first authority (root authority) */
  firstAuthorityType: number; // u16
  /** Length of first authority data */
  firstAuthorityDataLen: number; // u16
  /** Number of plugin refs for first authority */
  numPluginRefs: number; // u16
  /** RolePermission enum for first authority */
  rolePermission: number; // u8
}

/**
 * Sign instruction arguments
 * 
 * Layout: instruction_payload_len[2] + authority_id[4] = 6 bytes
 */
export interface SignArgs {
  /** Length of instruction payload (u16) */
  instructionPayloadLen: number;
  /** Authority ID performing the sign (u32) */
  authorityId: number;
}

/**
 * AddAuthority instruction arguments
 * 
 * Layout: acting_authority_id[4] + new_authority_type[2] + new_authority_data_len[2] + 
 *         num_plugin_refs[2] + role_permission[1] + padding[3] = 14 bytes
 */
export interface AddAuthorityArgs {
  /** Authority ID performing this action (u32) */
  actingAuthorityId: number;
  /** Type of new authority (u16) */
  newAuthorityType: number;
  /** Length of new authority data (u16) */
  newAuthorityDataLen: number;
  /** Number of plugin refs (u16) */
  numPluginRefs: number;
  /** RolePermission enum for new authority (u8) */
  rolePermission: number;
}

/**
 * UpdateAuthority instruction arguments
 */
export interface UpdateAuthorityArgs {
  /** Authority ID performing this action (u32) */
  actingAuthorityId: number;
  /** Authority ID to update (u32) */
  authorityToUpdateId: number;
  // Additional data follows (role_permission, plugin_refs, etc.)
}

/**
 * RemoveAuthority instruction arguments
 */
export interface RemoveAuthorityArgs {
  /** Authority ID performing this action (u32) */
  actingAuthorityId: number;
  /** Authority ID to remove (u32) */
  authorityToRemoveId: number;
}

/**
 * AddPlugin instruction arguments
 */
export interface AddPluginArgs {
  /** Authority ID performing this action (u32) */
  actingAuthorityId: number;
  // Plugin data follows
}

/**
 * RemovePlugin instruction arguments
 */
export interface RemovePluginArgs {
  /** Authority ID performing this action (u32) */
  actingAuthorityId: number;
  /** Plugin index to remove (u16) */
  pluginIndex: number;
}

/**
 * UpdatePlugin instruction arguments
 */
export interface UpdatePluginArgs {
  /** Authority ID performing this action (u32) */
  actingAuthorityId: number;
  /** Plugin index to update (u16) */
  pluginIndex: number;
  // Update data follows
}

/**
 * CreateSession instruction arguments
 */
export interface CreateSessionArgs {
  /** Authority ID creating the session (u32) */
  authorityId: number;
  /** Session key (32 bytes) */
  sessionKey: Uint8Array;
  /** Session duration in slots (u64) */
  duration: bigint;
}
