/**
 * Utility to pack instructions into the compact format expected by LazorKit's Execute instruction.
 * 
 * Format:
 * [num_instructions: u8]
 * for each instruction:
 *   [program_id_index: u8]
 *   [num_accounts: u8]
 *   [account_indexes: u8[]]
 *   [data_len: u16 LE]
 *   [data: u8[]]
 */

import { type AccountMeta } from "@solana/web3.js";

export interface CompactInstruction {

    programIdIndex: number;
    accountIndexes: number[];
    data: Uint8Array;
}

/**
 * Packs a list of compact instructions into a single buffer.
 * Used by the Execute instruction to encode inner instructions.
 */
export function packCompactInstructions(instructions: CompactInstruction[]): Uint8Array {
    if (instructions.length > 255) {
        throw new Error("Too many instructions (max 255)");
    }

    const buffers: Uint8Array[] = [];

    // 1. Number of instructions
    buffers.push(new Uint8Array([instructions.length]));

    for (const ix of instructions) {
        // 2. Program ID index + number of accounts
        if (ix.accountIndexes.length > 255) {
            throw new Error("Too many accounts in an instruction (max 255)");
        }
        buffers.push(new Uint8Array([ix.programIdIndex, ix.accountIndexes.length]));

        // 3. Account indexes
        buffers.push(new Uint8Array(ix.accountIndexes));

        // 4. Data length (u16 LE)
        const dataLen = ix.data.length;
        if (dataLen > 65535) {
            throw new Error("Instruction data too large (max 65535 bytes)");
        }
        buffers.push(new Uint8Array([dataLen & 0xff, (dataLen >> 8) & 0xff]));

        // 5. Data
        buffers.push(ix.data);
    }

    // Concatenate all buffers
    const totalLength = buffers.reduce((acc, b) => acc + b.length, 0);
    const result = new Uint8Array(totalLength);
    let offset = 0;
    for (const b of buffers) {
        result.set(b, offset);
        offset += b.length;
    }

    return result;
}


/**
 * Computes the SHA-256 hash of all account pubkeys referenced by compact instructions.
 * This MUST match the contract's `compute_accounts_hash` exactly.
 * 
 * @param accountMetas Array of absolute account metas used by the parent Execute instruction
 * @param instructions List of packed compact instructions
 */
export async function computeAccountsHash(
    accountMetas: AccountMeta[],
    instructions: CompactInstruction[]
): Promise<Uint8Array> {
    const pubkeysData: Uint8Array[] = [];

    for (const ix of instructions) {
        const programId = accountMetas[ix.programIdIndex].pubkey;
        pubkeysData.push(programId.toBytes());

        for (const idx of ix.accountIndexes) {
            if (idx >= accountMetas.length) {
                throw new Error(`Account index out of bounds: ${idx}`);
            }
            pubkeysData.push(accountMetas[idx].pubkey.toBytes());
        }
    }

    // Concatenate all pubkeys
    const totalLength = pubkeysData.reduce((acc, b) => acc + b.length, 0);
    const result = new Uint8Array(totalLength);
    let offset = 0;
    for (const b of pubkeysData) {
        result.set(b, offset);
        offset += b.length;
    }

    // Compute SHA-256 hash using Web Crypto API
    const hashBuffer = await crypto.subtle.digest("SHA-256", result as any);
    return new Uint8Array(hashBuffer);
}

