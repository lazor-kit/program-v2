import { AuthorityType } from './authority';
import { RolePermission } from './permission';
import type { PluginRef } from './plugin';

/**
 * Account discriminator types
 */
export enum Discriminator {
  /** Uninitialized account */
  Uninitialized = 0,
  /** Wallet Account (main account) */
  WalletAccount = 1,
}

/**
 * Wallet Account structure
 * 
 * Fixed header: 40 bytes
 * - discriminator: 1 byte
 * - bump: 1 byte
 * - id: 32 bytes
 * - wallet_bump: 1 byte
 * - version: 1 byte
 * - _reserved: 4 bytes
 */
export interface WalletAccount {
  /** Account type discriminator */
  discriminator: Discriminator;
  /** PDA bump seed */
  bump: number;
  /** Unique wallet identifier */
  id: Uint8Array; // 32 bytes
  /** Wallet vault PDA bump seed */
  walletBump: number;
  /** Account version */
  version: number;
}

/**
 * Authority data structure
 */
export interface AuthorityData {
  /** Authority type */
  authorityType: AuthorityType;
  /** Authority data bytes */
  authorityData: Uint8Array;
  /** Plugin references */
  pluginRefs: PluginRef[];
  /** Role permission */
  rolePermission: RolePermission;
  /** Authority ID */
  id: number;
}

/**
 * Wallet account constants
 */
export const WALLET_ACCOUNT_PREFIX = 'wallet_account';
export const WALLET_VAULT_PREFIX = 'wallet_vault';
export const WALLET_ACCOUNT_HEADER_SIZE = 40; // Fixed header size
export const NUM_AUTHORITIES_SIZE = 2; // u16
