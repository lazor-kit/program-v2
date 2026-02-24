/**
 * PDA derivation helpers for LazorKit accounts.
 * 
 * These are not auto-generated because the Shank IDL
 * doesn't include PDA seed definitions.
 */

import {
    getAddressEncoder,
    getProgramDerivedAddress,
    type Address,
    type ProgramDerivedAddress,
} from "@solana/kit";
import { LAZORKIT_PROGRAM_PROGRAM_ADDRESS } from "../generated";

const encoder = getAddressEncoder();

/**
 * Derives the Wallet PDA.
 * Seeds: ["wallet", user_seed]
 */
export async function findWalletPda(
    userSeed: Uint8Array
): Promise<ProgramDerivedAddress> {
    return await getProgramDerivedAddress({
        programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
        seeds: ["wallet", userSeed],
    });
}

/**
 * Derives the Vault PDA.
 * Seeds: ["vault", wallet_pubkey]
 */
export async function findVaultPda(
    wallet: Address
): Promise<ProgramDerivedAddress> {
    return await getProgramDerivedAddress({
        programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
        seeds: ["vault", encoder.encode(wallet)],
    });
}

/**
 * Derives an Authority PDA.
 * Seeds: ["authority", wallet_pubkey, id_seed]
 * @param idSeed - For Ed25519 this is the 32-byte public key. For Secp256r1 this is the 32-byte SHA256 Hash of the `credential_id` (rawId).
 */
export async function findAuthorityPda(
    wallet: Address,
    idSeed: Uint8Array
): Promise<ProgramDerivedAddress> {
    return await getProgramDerivedAddress({
        programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
        seeds: ["authority", encoder.encode(wallet), idSeed],
    });
}

/**
 * Derives a Session PDA.
 * Seeds: ["session", wallet_pubkey, session_key_pubkey]
 */
export async function findSessionPda(
    wallet: Address,
    sessionKey: Address
): Promise<ProgramDerivedAddress> {
    return await getProgramDerivedAddress({
        programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
        seeds: ["session", encoder.encode(wallet), encoder.encode(sessionKey)],
    });
}
