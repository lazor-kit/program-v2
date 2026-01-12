import type { Address } from '@solana/kit';
import { AuthorityType } from '../types';

/**
 * Base interface for all authority implementations
 */
export interface Authority {
  /** Authority type */
  type: AuthorityType;
  
  /** Get the public key or address for this authority */
  getPublicKey(): Address | Uint8Array | Promise<Address>;
  
  /** Sign a message (for Ed25519) */
  sign?(message: Uint8Array): Promise<Uint8Array>;
  
  /** Get current odometer (for Secp256k1/Secp256r1) */
  getOdometer?(): Promise<number>;
  
  /** Increment odometer (for Secp256k1/Secp256r1) */
  incrementOdometer?(): Promise<void>;
  
  /** Serialize authority data for on-chain storage */
  serialize(): Promise<Uint8Array>;
}
