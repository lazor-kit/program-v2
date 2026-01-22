import { address, Address, getAddressEncoder, getProgramDerivedAddress, ReadonlyUint8Array } from '@solana/kit';
import { sha256 } from '@noble/hashes/sha256';
import { Point } from '@noble/ed25519';

/**
 * LazorKit program ID
 */
export const LAZORKIT_PROGRAM_ID = address('8BkWE4RTAFmptSjg3AEsgdfbGtsg9i32ia723wqdfkaX');

const MAX_SEED_LENGTH = 32;
const PDA_MARKER = Buffer.from('ProgramDerivedAddress');

/**
 * PDA result
 */
export interface PDA {
    address: Address;
    bump: number;
}

/**
 * Find program derived address
 */
async function findProgramAddress(
    seeds: (Uint8Array | ReadonlyUint8Array)[],
    programId: Address
): Promise<PDA> {
    // Use Kit's getProgramDerivedAddress which handles bump iteration and off-curve check
    const [pdaAddress, bump] = await getProgramDerivedAddress({
        programAddress: programId,
        seeds: seeds,
    });

    return {
        address: pdaAddress,
        bump,
    };
}

/**
 * Find config PDA
 */
export async function findConfigPDA(walletId: Uint8Array): Promise<PDA> {
    if (walletId.length !== 32) {
        throw new Error('Wallet ID must be 32 bytes');
    }

    return findProgramAddress(
        [new TextEncoder().encode('lazorkit'), walletId],
        LAZORKIT_PROGRAM_ID
    );
}

/**
 * Find vault PDA
 */
export async function findVaultPDA(configAddress: Address): Promise<PDA> {
    const addressEncoder = getAddressEncoder();
    const configBytes = addressEncoder.encode(configAddress);

    // Ensure configBytes is passed as compatible type
    // Explicitly convert readonly bytes to Uint8Array for API compatibility if needed
    return findProgramAddress(
        [new TextEncoder().encode('lazorkit-wallet-address'), new Uint8Array(configBytes)],
        LAZORKIT_PROGRAM_ID
    );
}

/**
 * Generate random wallet ID
 */
export function generateWalletId(): Uint8Array {
    return crypto.getRandomValues(new Uint8Array(32));
}
