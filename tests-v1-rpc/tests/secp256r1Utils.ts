/**
 * Test-only Secp256r1 mock utilities for LazorKit V1 tests.
 *
 * This file provides:
 *   - A mock signer that implements the SDK's `Secp256r1Signer` interface using Web Crypto API.
 *   - Re-exports of SDK functions so tests use the real SDK path.
 */

import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import { type Secp256r1Signer, buildSecp256r1Message, buildAuthenticatorData, buildAuthPayload, buildSecp256r1PrecompileIx, appendSecp256r1Sysvars, readCurrentSlot } from "@lazorkit/solita-client";

// Re-export SDK functions so tests can just import them from here
export {
    buildAuthenticatorData,
    buildAuthPayload,
    buildSecp256r1Message,
    buildSecp256r1PrecompileIx,
    appendSecp256r1Sysvars,
    readCurrentSlot
};

// ─── Mock Signer ─────────────────────────────────────────────────────────────

interface MockSecp256r1Signer extends Secp256r1Signer {
    privateKey: CryptoKey;
}

/**
 * Generates a mock Secp256r1 signer that implements the SDK's `Secp256r1Signer` interface.
 * Uses Web Crypto API for key generation and signing.
 */
export async function generateMockSecp256r1Signer(credentialIdHash?: Uint8Array): Promise<MockSecp256r1Signer> {
    const keyPair = await crypto.subtle.generateKey(
        { name: "ECDSA", namedCurve: "P-256" },
        true,
        ["sign", "verify"]
    );

    const spki = await crypto.subtle.exportKey("spki", keyPair.publicKey);
    const rawP256Pubkey = new Uint8Array(spki).slice(-64);
    const compressedPubKey = new Uint8Array(33);
    compressedPubKey[0] = (rawP256Pubkey[63] % 2 === 0) ? 0x02 : 0x03;
    compressedPubKey.set(rawP256Pubkey.slice(0, 32), 1);

    const credHash = credentialIdHash || new Uint8Array(32).map(() => Math.floor(Math.random() * 256));

    return {
        privateKey: keyPair.privateKey,
        publicKeyBytes: compressedPubKey,
        credentialIdHash: credHash,
        sign: async (message: Uint8Array): Promise<Uint8Array> => {
            return signWithLowS(keyPair.privateKey, message);
        },
    };
}

// ─── Low-S signing helper ────────────────────────────────────────────────────

/**
 * Signs a message with the given private key and enforces low-S as required
 * by the Solana Secp256r1 precompile.
 */
async function signWithLowS(privateKey: CryptoKey, message: Uint8Array): Promise<Uint8Array> {
    // Note: crypto.subtle.sign("ECDSA") automatically hashes the message using the specified hash
    const rawSigBuffer = await crypto.subtle.sign(
        { name: "ECDSA", hash: "SHA-256" },
        privateKey,
        message as any
    );
    const rawSig = new Uint8Array(rawSigBuffer);

    // crypto.subtle outputs exactly 64 bytes (r || s)
    if (rawSig.length !== 64) {
        throw new Error(`Unexpected signature length from crypto.subtle: ${rawSig.length}`);
    }

    // SECP256R1 curve order n
    const SECP256R1_N = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551n;
    const HALF_N = SECP256R1_N / 2n;

    const rBuffer = rawSig.slice(0, 32);
    const sBufferLocal = rawSig.slice(32, 64);

    // Convert s to bigint and enforce low-S
    let sBigInt = 0n;
    for (let i = 0; i < 32; i++) {
        sBigInt = (sBigInt << 8n) + BigInt(sBufferLocal[i]);
    }

    if (sBigInt > HALF_N) {
        sBigInt = SECP256R1_N - sBigInt;
    }

    const modifiedSBuffer = new Uint8Array(32);
    for (let i = 31; i >= 0; i--) {
        modifiedSBuffer[i] = Number(sBigInt & 0xffn);
        sBigInt >>= 8n;
    }

    const lowSSig = new Uint8Array(64);
    lowSSig.set(rBuffer, 0);
    lowSSig.set(modifiedSBuffer, 32);

    return lowSSig;
}
