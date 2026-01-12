import { type Address, type Instruction, type AccountMeta, AccountRole } from '@solana/kit';
import { PublicKey } from '@solana/web3.js'; // Keep for buffer conversion utilities if needed, or use kit

// ============================================================================
// SOL LIMIT PLUGIN
// ============================================================================

export class SolLimit {
    static PROGRAM_ID: Address;

    constructor(programId: Address) {
        SolLimit.PROGRAM_ID = programId;
    }

    static async createInitConfigInstruction(params: {
        payer: Address;
        configAccount: Address;
        limit: bigint; // lamports
        programId: Address;
    }): Promise<Instruction> {

        // Serialization for [instruction: u8, limit: u64]
        const data = Buffer.alloc(9);
        data.writeUInt8(1, 0); // InitConfig = 1
        data.writeBigUInt64LE(params.limit, 1);

        return {
            programAddress: params.programId,
            accounts: [
                { address: params.payer, role: AccountRole.WRITABLE }, // Signer handled externally
                { address: params.configAccount, role: AccountRole.WRITABLE },
                { address: '11111111111111111111111111111111' as Address, role: AccountRole.READONLY }, // System Program
            ],
            data,
        };
    }
}

// ============================================================================
// PROGRAM WHITELIST PLUGIN
// ============================================================================

export class ProgramWhitelist {
    static PROGRAM_ID: Address;

    constructor(programId: Address) {
        ProgramWhitelist.PROGRAM_ID = programId;
    }

    static async createInitConfigInstruction(params: {
        payer: Address;
        configAccount: Address;
        programIds: Address[];
        programId: Address;
    }): Promise<Instruction> {

        // Manual serialization to ensure compatibility
        // InitConfig discriminator (1 byte) + Vec len (4 bytes) + (numIds * 32 bytes)
        const numIds = params.programIds.length;
        const buffer = Buffer.alloc(1 + 4 + (numIds * 32));
        buffer.writeUInt8(1, 0); // InitConfig
        buffer.writeUInt32LE(numIds, 1); // Vec len

        let offset = 5;
        for (const id of params.programIds) {
            const pubkeyBytes = new PublicKey(id).toBuffer();
            pubkeyBytes.copy(buffer, offset);
            offset += 32;
        }

        return {
            programAddress: params.programId,
            accounts: [
                { address: params.payer, role: AccountRole.WRITABLE },
                { address: params.configAccount, role: AccountRole.WRITABLE },
                { address: '11111111111111111111111111111111' as Address, role: AccountRole.READONLY },
            ],
            data: buffer,
        };
    }
}
