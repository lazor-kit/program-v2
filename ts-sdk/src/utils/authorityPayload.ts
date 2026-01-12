import { AuthorityType } from '../types';
import type { Authority } from '../authority/base';
import { LazorkitError, LazorkitErrorCode } from '../errors';

/**
 * Build authority payload for signing
 * 
 * The authority payload structure depends on the authority type:
 * - Ed25519: signature[64 bytes]
 * - Secp256k1: signature[64 bytes] + odometer[4 bytes] + slot[8 bytes] = 76 bytes
 * - Secp256r1: signature[64 bytes] + odometer[4 bytes] + slot[8 bytes] = 76 bytes
 * - Session-based: session_signature[64 bytes] + (odometer + slot if applicable)
 */
export async function buildAuthorityPayload(params: {
  authority: Authority;
  message: Uint8Array;
  odometer?: number;
  slot?: bigint;
}): Promise<Uint8Array> {
  const { authority, message, odometer, slot } = params;

  switch (authority.type) {
    case AuthorityType.Ed25519:
      // Ed25519: Just signature (64 bytes)
      if (!authority.sign) {
        throw new LazorkitError(
          LazorkitErrorCode.InvalidAuthorityType,
          'Ed25519 authority must support signing'
        );
      }
      const signature = await authority.sign(message);
      if (signature.length !== 64) {
        throw new LazorkitError(
          LazorkitErrorCode.SerializationError,
          `Ed25519 signature must be 64 bytes, got ${signature.length}`
        );
      }
      return signature;

    case AuthorityType.Secp256k1:
    case AuthorityType.Secp256r1:
      // Secp256k1/Secp256r1: signature[64] + odometer[4] + slot[8] = 76 bytes
      if (!authority.sign) {
        throw new LazorkitError(
          LazorkitErrorCode.InvalidAuthorityType,
          'Secp256k1/Secp256r1 authority must support signing'
        );
      }
      if (odometer === undefined) {
        throw new LazorkitError(
          LazorkitErrorCode.SerializationError,
          'Odometer is required for Secp256k1/Secp256r1 authorities'
        );
      }
      if (slot === undefined) {
        throw new LazorkitError(
          LazorkitErrorCode.SerializationError,
          'Slot is required for Secp256k1/Secp256r1 authorities'
        );
      }

      const secpSignature = await authority.sign(message);
      if (secpSignature.length !== 64) {
        throw new LazorkitError(
          LazorkitErrorCode.SerializationError,
          `Secp256k1/Secp256r1 signature must be 64 bytes, got ${secpSignature.length}`
        );
      }

      // Build payload: signature[64] + odometer[4] + slot[8]
      const payload = new Uint8Array(76);
      payload.set(secpSignature, 0);
      
      // Write odometer (little-endian u32)
      payload[64] = odometer & 0xff;
      payload[65] = (odometer >> 8) & 0xff;
      payload[66] = (odometer >> 16) & 0xff;
      payload[67] = (odometer >> 24) & 0xff;
      
      // Write slot (little-endian u64)
      let slotValue = slot;
      for (let i = 0; i < 8; i++) {
        payload[68 + i] = Number(slotValue & 0xffn);
        slotValue = slotValue >> 8n;
      }

      return payload;

    case AuthorityType.Ed25519Session:
    case AuthorityType.Secp256k1Session:
    case AuthorityType.Secp256r1Session:
      // Session-based: session_signature[64] + (odometer + slot if Secp256k1/Secp256r1)
      if (!authority.sign) {
        throw new LazorkitError(
          LazorkitErrorCode.InvalidAuthorityType,
          'Session authority must support signing'
        );
      }
      
      const sessionSignature = await authority.sign(message);
      if (sessionSignature.length !== 64) {
        throw new LazorkitError(
          LazorkitErrorCode.SerializationError,
          `Session signature must be 64 bytes, got ${sessionSignature.length}`
        );
      }

      // For Secp256k1Session/Secp256r1Session, include odometer and slot
      if (authority.type === AuthorityType.Secp256k1Session || 
          authority.type === AuthorityType.Secp256r1Session) {
        if (odometer === undefined || slot === undefined) {
          throw new LazorkitError(
            LazorkitErrorCode.SerializationError,
            'Odometer and slot are required for Secp256k1Session/Secp256r1Session'
          );
        }

        const sessionPayload = new Uint8Array(76);
        sessionPayload.set(sessionSignature, 0);
        
        // Write odometer
        sessionPayload[64] = odometer & 0xff;
        sessionPayload[65] = (odometer >> 8) & 0xff;
        sessionPayload[66] = (odometer >> 16) & 0xff;
        sessionPayload[67] = (odometer >> 24) & 0xff;
        
        // Write slot
        let slotVal = slot;
        for (let i = 0; i < 8; i++) {
          sessionPayload[68 + i] = Number(slotVal & 0xffn);
          slotVal = slotVal >> 8n;
        }

        return sessionPayload;
      }

      // Ed25519Session: Just signature
      return sessionSignature;

    default:
      throw new LazorkitError(
        LazorkitErrorCode.InvalidAuthorityType,
        `Unsupported authority type: ${authority.type}`
      );
  }
}

/**
 * Build message hash for Secp256k1/Secp256r1 signing
 * 
 * The message includes:
 * - instruction_payload
 * - odometer (for Secp256k1/Secp256r1)
 * - slot (for Secp256k1/Secp256r1)
 */
export async function buildMessageHash(params: {
  instructionPayload: Uint8Array;
  odometer?: number;
  slot?: bigint;
  authorityType: AuthorityType;
}): Promise<Uint8Array> {
  const { instructionPayload, odometer, slot, authorityType } = params;

  // For Ed25519, just hash the instruction payload
  if (authorityType === AuthorityType.Ed25519 || 
      authorityType === AuthorityType.Ed25519Session) {
    // Use SHA-256 for Ed25519 (Solana standard)
    // Note: In practice, you'd use a proper hashing library
    // This is a simplified version
    return await hashSha256(instructionPayload);
  }

  // For Secp256k1/Secp256r1, include odometer and slot
  if (authorityType === AuthorityType.Secp256k1 || 
      authorityType === AuthorityType.Secp256r1 ||
      authorityType === AuthorityType.Secp256k1Session ||
      authorityType === AuthorityType.Secp256r1Session) {
    if (odometer === undefined || slot === undefined) {
      throw new LazorkitError(
        LazorkitErrorCode.SerializationError,
        'Odometer and slot are required for Secp256k1/Secp256r1'
      );
    }

    // Build message: instruction_payload + odometer[4] + slot[8]
    const message = new Uint8Array(instructionPayload.length + 12);
    message.set(instructionPayload, 0);
    
    // Write odometer (little-endian u32)
    message[instructionPayload.length] = odometer & 0xff;
    message[instructionPayload.length + 1] = (odometer >> 8) & 0xff;
    message[instructionPayload.length + 2] = (odometer >> 16) & 0xff;
    message[instructionPayload.length + 3] = (odometer >> 24) & 0xff;
    
    // Write slot (little-endian u64)
    let slotValue = slot;
    for (let i = 0; i < 8; i++) {
      message[instructionPayload.length + 4 + i] = Number(slotValue & 0xffn);
      slotValue = slotValue >> 8n;
    }

    // Use Keccak256 for Secp256k1, SHA-256 for Secp256r1
    // Note: In practice, you'd use proper hashing libraries
    if (authorityType === AuthorityType.Secp256k1 || 
        authorityType === AuthorityType.Secp256k1Session) {
      return await hashKeccak256(message);
    } else {
      return await hashSha256(message);
    }
  }

  throw new LazorkitError(
    LazorkitErrorCode.InvalidAuthorityType,
    `Unsupported authority type: ${authorityType}`
  );
}

/**
 * Hash using SHA-256
 * 
 * Note: This is a placeholder. In production, use a proper crypto library.
 */
async function hashSha256(data: Uint8Array): Promise<Uint8Array> {
  const hash = await crypto.subtle.digest('SHA-256', data.buffer as ArrayBuffer);
  return new Uint8Array(hash);
}

/**
 * Hash using Keccak256
 * 
 * Note: This is a placeholder. In production, use a proper Keccak256 library.
 * Web Crypto API doesn't support Keccak256, so you'd need a library like js-sha3.
 */
async function hashKeccak256(_data: Uint8Array): Promise<Uint8Array> {
  // Placeholder - would need js-sha3 or similar
  // For now, throw error to indicate this needs implementation
  throw new LazorkitError(
    LazorkitErrorCode.SerializationError,
    'Keccak256 hashing not yet implemented. Please use a library like js-sha3.'
  );
}
