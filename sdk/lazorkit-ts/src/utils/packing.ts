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
