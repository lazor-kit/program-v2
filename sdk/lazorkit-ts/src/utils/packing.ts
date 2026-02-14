
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
 * Packs a list of compact instructions into a buffer.
 */
export function packCompactInstructions(instructions: CompactInstruction[]): Uint8Array {
    if (instructions.length > 255) {
        throw new Error("Too many instructions (max 255)");
    }

    const buffers: Uint8Array[] = [];

    // 1. Number of instructions
    const header = new Uint8Array([instructions.length]);
    buffers.push(header);

    for (const ix of instructions) {
        // 2. Program ID index
        const ixHeader = new Uint8Array(2);
        ixHeader[0] = ix.programIdIndex;

        // 3. Number of accounts
        if (ix.accountIndexes.length > 255) {
            throw new Error("Too many accounts in an instruction (max 255)");
        }
        ixHeader[1] = ix.accountIndexes.length;
        buffers.push(ixHeader);

        // 4. Account indexes
        buffers.push(new Uint8Array(ix.accountIndexes));

        // 5. Data length (u16 LE)
        const dataLen = ix.data.length;
        if (dataLen > 65535) {
            throw new Error("Instruction data too large (max 65535 bytes)");
        }
        const lenBuffer = new Uint8Array(2);
        lenBuffer[0] = dataLen & 0xff;
        lenBuffer[1] = (dataLen >> 8) & 0xff;
        buffers.push(lenBuffer);

        // 6. Data
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
