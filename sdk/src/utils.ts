import { getProgramDerivedAddress, Address, getAddressEncoder } from "@solana/addresses";
import { LAZORKIT_PROGRAM_ID, SEEDS } from "./constants";
import { ReadonlyUint8Array } from "@solana/codecs";

const ENCODER = new TextEncoder();

export async function findWalletPDA(
    userSeed: Uint8Array,
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<readonly [Address, number]> {
    // Seeds: ["wallet", user_seed]
    return await getProgramDerivedAddress({
        programAddress: programId,
        seeds: [ENCODER.encode(SEEDS.WALLET), userSeed],
    });
}

export async function findVaultPDA(
    walletPda: Address,
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<readonly [Address, number]> {
    // Seeds: ["vault", wallet_pda]
    // Note: getProgramDerivedAddress expects seeds as Uint8Array or string.
    // Address is a string, but the contract expects the 32-byte public key bytes.
    // We need to decode the address string to bytes for the seed.
    // However, the modern SDK handles `Address` in seeds usually by requiring conversion if using raw bytes.
    // Wait, `getProgramDerivedAddress` seeds are `ReadonlyArray<Uint8Array | string>`.
    // If we pass string, it is UTF-8 encoded. We need raw bytes of the address.
    // But wait, the @solana/addresses package should have a helper or we need to encode it.

    // Actually, for address bytes in seeds, we strictly need the 32 bytes.
    // We can use `getAddressEncoder().encode(address)` from @solana/addresses or codecs?
    // Let's use generic codec.

    const walletBytes = getAddressBytes(walletPda);

    return await getProgramDerivedAddress({
        programAddress: programId,
        seeds: [ENCODER.encode(SEEDS.VAULT), walletBytes],
    });
}

export async function findAuthorityPDA(
    walletPda: Address,
    idSeed: Uint8Array, // 32 bytes
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<readonly [Address, number]> {
    const walletBytes = getAddressBytes(walletPda);

    return await getProgramDerivedAddress({
        programAddress: programId,
        seeds: [ENCODER.encode(SEEDS.AUTHORITY), walletBytes, idSeed],
    });
}

export async function findSessionPDA(
    walletPda: Address,
    sessionKeyBytes: Uint8Array, // 32 bytes
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<readonly [Address, number]> {
    const walletBytes = getAddressBytes(walletPda);

    return await getProgramDerivedAddress({
        programAddress: programId,
        seeds: [ENCODER.encode(SEEDS.SESSION), walletBytes, sessionKeyBytes],
    });
}

// Helper to convert Address string to Unit8Array (32 bytes)
export function getAddressBytes(addr: Address): ReadonlyUint8Array {
    return getAddressEncoder().encode(addr);
}
