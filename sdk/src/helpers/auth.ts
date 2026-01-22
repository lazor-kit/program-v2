import * as ed25519 from '@noble/ed25519';

/**
 * Authority types supported by LazorKit
 */
export enum AuthorityType {
    Ed25519 = 1,
    Ed25519Session = 2,
    Secp256r1 = 5,
    Secp256r1Session = 6,
}

/**
 * Role types in LazorKit RBAC
 */
export enum RoleType {
    Owner = 0,
    Admin = 1,
    Spender = 2,
}

/**
 * Encode an Ed25519 public key as authority data
 * @param publicKey - Ed25519 public key (32 bytes)
 * @returns Authority data bytes
 */
export function encodeEd25519Authority(publicKey: Uint8Array): Uint8Array {
    if (publicKey.length !== 32) {
        throw new Error('Ed25519 public key must be 32 bytes');
    }
    return publicKey;
}

/**
 * Encode an Ed25519 session authority
 * @param masterKey - Master Ed25519 public key (32 bytes)
 * @param sessionKey - Session Ed25519 public key (32 bytes)
 * @param validUntil - Slot when session expires
 * @returns Authority data bytes (72 bytes)
 */
export function encodeEd25519SessionAuthority(
    masterKey: Uint8Array,
    sessionKey: Uint8Array,
    validUntil: bigint
): Uint8Array {
    if (masterKey.length !== 32 || sessionKey.length !== 32) {
        throw new Error('Keys must be 32 bytes each');
    }

    const buffer = new Uint8Array(72); // 32 + 32 + 8
    buffer.set(masterKey, 0);
    buffer.set(sessionKey, 32);

    const view = new DataView(buffer.buffer);
    view.setBigUint64(64, validUntil, true); // little-endian

    return buffer;
}

/**
 * Encode a Secp256r1 public key as authority data
 * @param publicKey - Secp256r1 public key (compressed, 33 bytes)
 * @returns Authority data bytes
 */
export function encodeSecp256r1Authority(publicKey: Uint8Array): Uint8Array {
    if (publicKey.length !== 33) {
        throw new Error('Secp256r1 public key must be 33 bytes (compressed)');
    }
    return publicKey;
}

/**
 * Create Ed25519 signature for authorization
 * @param privateKey - Ed25519 private key (32 bytes)
 * @param message - Message to sign
 * @returns Signature (64 bytes)
 */
export async function createEd25519Signature(
    privateKey: Uint8Array,
    message: Uint8Array
): Promise<Uint8Array> {
    return ed25519.sign(message, privateKey);
}

/**
 * Get authority data length for a given authority type
 * @param authorityType - Authority type enum
 * @returns Expected data length in bytes
 */
export function getAuthorityDataLength(authorityType: AuthorityType): number {
    switch (authorityType) {
        case AuthorityType.Ed25519:
            return 32;
        case AuthorityType.Ed25519Session:
            return 72; // 32 + 32 + 8
        case AuthorityType.Secp256r1:
            return 33;
        case AuthorityType.Secp256r1Session:
            return 73; // 33 + 32 + 8
        default:
            throw new Error(`Unknown authority type: ${authorityType}`);
    }
}
