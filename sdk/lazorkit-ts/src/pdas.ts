
import {
    getAddressEncoder,
    getProgramDerivedAddress,
    Address,
    ProgramDerivedAddress
} from "@solana/kit";
import { LAZOR_KIT_PROGRAM_ID } from "./constants";

const encoder = getAddressEncoder();

/**
 * Derives the Wallet PDA.
 * Seeds: ["wallet", user_seed]
 */
export async function findWalletPda(
    userSeed: Uint8Array
): Promise<ProgramDerivedAddress> {
    return await getProgramDerivedAddress({
        programAddress: LAZOR_KIT_PROGRAM_ID,
        seeds: [
            "wallet",
            userSeed
        ],
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
        programAddress: LAZOR_KIT_PROGRAM_ID,
        seeds: [
            "vault",
            encoder.encode(wallet)
        ],
    });
}

/**
 * Derives an Authority PDA.
 * Seeds: ["authority", wallet_pubkey, id_hash]
 */
export async function findAuthorityPda(
    wallet: Address,
    idHash: Uint8Array
): Promise<ProgramDerivedAddress> {
    return await getProgramDerivedAddress({
        programAddress: LAZOR_KIT_PROGRAM_ID,
        seeds: [
            "authority",
            encoder.encode(wallet),
            idHash
        ],
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
        programAddress: LAZOR_KIT_PROGRAM_ID,
        seeds: [
            "session",
            encoder.encode(wallet),
            encoder.encode(sessionKey)
        ],
    });
}
