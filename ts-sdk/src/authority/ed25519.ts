import { AuthorityType } from '../types';
import type { Authority } from './base';
import { getAddressFromPublicKey, getPublicKeyFromAddress } from '@solana/kit';
import type { Address } from '@solana/kit';

/**
 * Ed25519 authority implementation
 * 
 * Uses Web Crypto API CryptoKey for signing
 */
export class Ed25519Authority implements Authority {
  type = AuthorityType.Ed25519;
  
  // Cache for public key bytes to avoid repeated exports
  private publicKeyBytes?: Uint8Array;
  // Cache for address to avoid repeated conversions
  private cachedAddress?: Address;
  
  constructor(
    private publicKey: CryptoKey,
    private privateKey?: CryptoKey,
    publicKeyBytes?: Uint8Array, // Optional: provide public key bytes directly to avoid export issues
    cachedAddress?: Address // Optional: provide address directly to avoid conversion issues
  ) {
    if (publicKeyBytes && publicKeyBytes.length === 32) {
      this.publicKeyBytes = publicKeyBytes;
    }
    if (cachedAddress) {
      this.cachedAddress = cachedAddress;
    }
  }

  /**
   * Create from CryptoKeyPair
   * 
   * Note: This will attempt to export the public key bytes.
   * If export fails, we'll use address decoding as fallback.
   */
  static async fromKeyPair(keyPair: CryptoKeyPair): Promise<Ed25519Authority> {
    // Try to export public key bytes immediately
    let publicKeyBytes: Uint8Array | undefined;
    let cachedAddress: Address | undefined;
    
    try {
      const exported = await crypto.subtle.exportKey('raw', keyPair.publicKey);
      const bytes = new Uint8Array(exported);
      if (bytes.length === 32) {
        publicKeyBytes = bytes;
      }
    } catch (exportError) {
      // Export failed - try to get address and decode it
      try {
        cachedAddress = await getAddressFromPublicKey(keyPair.publicKey);
        // If we got address, decode it to get public key bytes
        const bs58 = await import('bs58');
        const addressString = String(cachedAddress);
        const decoded = bs58.default.decode(addressString);
        const bytes = new Uint8Array(decoded);
        if (bytes.length === 32) {
          publicKeyBytes = bytes;
        }
      } catch (addressError) {
        // Both export and address conversion failed
        // Failed to export and get address - will try again in serialize()
      }
    }
    
    return new Ed25519Authority(keyPair.publicKey, keyPair.privateKey, publicKeyBytes, cachedAddress);
  }

  /**
   * Create from public key only (read-only)
   */
  static async fromPublicKey(publicKey: CryptoKey): Promise<Ed25519Authority> {
    return new Ed25519Authority(publicKey);
  }

  /**
   * Create from address string
   */
  static async fromAddress(address: Address): Promise<Ed25519Authority> {
    const publicKey = await getPublicKeyFromAddress(address);
    return new Ed25519Authority(publicKey);
  }

  async getPublicKey(): Promise<Address> {
    // Return cached address if available
    if (this.cachedAddress) {
      return this.cachedAddress;
    }
    
    // Try to get address from CryptoKey
    try {
      const address = await getAddressFromPublicKey(this.publicKey);
      this.cachedAddress = address; // Cache it
      return address;
    } catch (error) {
      throw new Error(
        `Failed to get address from CryptoKey: ${error instanceof Error ? error.message : String(error)}. ` +
        `The CryptoKey may not be a valid Ed25519 public key.`
      );
    }
  }

  /**
   * Get public key as raw bytes (32 bytes for Ed25519)
   * This is a helper method for serialization
   */
  async getPublicKeyBytes(): Promise<Uint8Array> {
    // Return cached bytes if available
    if (this.publicKeyBytes) {
      return this.publicKeyBytes;
    }
    
    try {
      // Try to export as raw bytes (works in Node.js 20+ with Ed25519 support)
      const exported = await crypto.subtle.exportKey('raw', this.publicKey);
      const publicKeyBytes = new Uint8Array(exported);
      
      if (publicKeyBytes.length === 32) {
        this.publicKeyBytes = publicKeyBytes; // Cache it
        return publicKeyBytes;
      }
      
      throw new Error(`Invalid Ed25519 public key length: ${publicKeyBytes.length}, expected 32`);
    } catch (error) {
      // Fallback: Get address and decode from base58
      // Address is base58-encoded public key, so we decode it to get the 32-byte public key
      try {
        // Use cached address if available, otherwise try to get it
        let address: Address;
        if (this.cachedAddress) {
          address = this.cachedAddress;
        } else {
          try {
            address = await this.getPublicKey();
          } catch (getAddressError) {
            // If getPublicKey fails, we can't decode the address
            throw new Error(
              `Cannot get address from CryptoKey: ${getAddressError instanceof Error ? getAddressError.message : String(getAddressError)}. ` +
              `This usually means the CryptoKey is not a valid Ed25519 key.`
            );
          }
        }
        
        const addressString = String(address);
        
        // Use bs58 to decode the base58-encoded address
        const bs58 = await import('bs58');
        const decoded = bs58.default.decode(addressString);
        const publicKeyBytes = new Uint8Array(decoded);
        
        if (publicKeyBytes.length === 32) {
          this.publicKeyBytes = publicKeyBytes; // Cache it
          return publicKeyBytes;
        }
        
        throw new Error(`Decoded address length is ${publicKeyBytes.length}, expected 32 bytes`);
      } catch (decodeError) {
        throw new Error(
          `Failed to serialize Ed25519 authority: ${error instanceof Error ? error.message : String(error)}. ` +
          `Decode fallback also failed: ${decodeError instanceof Error ? decodeError.message : String(decodeError)}`
        );
      }
    }
  }

  async sign(message: Uint8Array): Promise<Uint8Array> {
    if (!this.privateKey) {
      throw new Error('Private key not available for signing');
    }
    
    // Sign using Web Crypto API
    const signature = await crypto.subtle.sign(
      {
        name: 'Ed25519',
      },
      this.privateKey,
      message.buffer as ArrayBuffer
    );
    
    return new Uint8Array(signature);
  }

  /**
   * Serialize Ed25519 authority data (32-byte public key)
   */
  async serialize(): Promise<Uint8Array> {
    return await this.getPublicKeyBytes();
  }
}
