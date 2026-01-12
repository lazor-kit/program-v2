import type { Address } from '@solana/kit';

/**
 * Plugin reference structure
 * 
 * Links authority to a plugin in the registry
 */
export interface PluginRef {
  /** Index in plugin registry */
  pluginIndex: number; // u16
  /** Priority (0 = highest) */
  priority: number; // u8
  /** Enabled flag */
  enabled: boolean;
}

/**
 * Plugin entry in registry
 */
export interface PluginEntry {
  /** Plugin program ID */
  programId: Address;
  /** Plugin config account */
  configAccount: Address;
  /** Priority (0 = highest) */
  priority: number;
  /** Enabled flag */
  enabled: boolean;
}
