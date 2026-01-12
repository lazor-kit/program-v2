import { AuthorityType } from '../types';
import { LazorkitError, LazorkitErrorCode } from '../errors';

/**
 * Generate a random session key (32 bytes)
 */
export function generateSessionKey(): Uint8Array {
  const key = new Uint8Array(32);
  crypto.getRandomValues(key);
  return key;
}

/**
 * Create session key from Ed25519 public key
 * 
 * For Ed25519Session, the session key is typically the Ed25519 public key
 */
export async function createSessionKeyFromEd25519(_publicKey: CryptoKey): Promise<Uint8Array> {
  // Export public key to bytes
  // Note: This is a simplified version - in practice, you'd need to properly export the key
  throw new LazorkitError(
    LazorkitErrorCode.SerializationError,
    'Session key creation from Ed25519 not yet fully implemented. Use generateSessionKey() for now.'
  );
}

/**
 * Calculate session expiration slot
 * 
 * @param currentSlot - Current slot number
 * @param duration - Session duration in slots
 * @returns Expiration slot
 */
export function calculateSessionExpiration(currentSlot: bigint, duration: bigint): bigint {
  return currentSlot + duration;
}

/**
 * Check if a session is expired
 * 
 * @param expirationSlot - Session expiration slot
 * @param currentSlot - Current slot number
 * @returns True if session is expired
 */
export function isSessionExpired(expirationSlot: bigint, currentSlot: bigint): boolean {
  return currentSlot > expirationSlot;
}

/**
 * Get recommended session duration based on authority type
 * 
 * @param authorityType - Authority type
 * @returns Recommended duration in slots (default: 1000 slots ~ 1 minute at 400ms/slot)
 */
export function getRecommendedSessionDuration(authorityType: AuthorityType): bigint {
  // Default: 1000 slots (~1 minute at 400ms per slot)
  // Adjust based on security requirements
  switch (authorityType) {
    case AuthorityType.Ed25519Session:
    case AuthorityType.Secp256k1Session:
    case AuthorityType.Secp256r1Session:
      return 1000n; // 1 minute
    default:
      return 1000n;
  }
}
