import { start } from "solana-bankrun";
import { PublicKey, Keypair, Transaction, TransactionInstruction, SystemProgram } from "@solana/web3.js";
import { Address, AccountRole } from "@solana/kit";
import { LazorClient } from "../src";

export const PROGRAM_ID_STR = "Btg4mLUdMd3ov8PBtmuuFMAimLAdXyew9XmsGtuY9VcP";
export const PROGRAM_ID = new PublicKey(PROGRAM_ID_STR);

class BankrunRpcAdapter {
    constructor(private banksClient: any) { }
    getAccountInfo(address: Address) {
        return {
            send: async () => {
                const acc = await this.banksClient.getAccount(new PublicKey(address));
                if (!acc) return { value: null };
                return {
                    value: {
                        data: [Buffer.from(acc.data).toString("base64"), "base64"],
                        executable: acc.executable,
                        lamports: BigInt(acc.lamports),
                        owner: acc.owner.toBase58(),
                    }
                };
            }
        };
    }
}

export async function setupTest(): Promise<{ context: any, client: LazorClient }> {
    const context = await start(
        [{ name: "lazorkit_program", programId: PROGRAM_ID }],
        []
    );
    const rpc = new BankrunRpcAdapter(context.banksClient);
    const client = new LazorClient(rpc as any);
    return { context, client };
}

export async function processTransaction(context: any, ixs: TransactionInstruction[], signers: Keypair[]) {
    const tx = new Transaction();
    tx.recentBlockhash = (await context.banksClient.getLatestBlockhash())[0];
    tx.feePayer = context.payer.publicKey;
    ixs.forEach(ix => tx.add(ix));
    tx.sign(context.payer, ...signers);

    const result = await context.banksClient.processTransaction(tx);
    return result;
}

export async function processInstruction(context: any, ix: any, signers: Keypair[] = [], extraAccounts: any[] = []) {
    const keys = [
        ...ix.accounts.map((a: any) => ({
            pubkey: new PublicKey(a.address),
            isSigner: !!(a.role & 0x02),
            isWritable: !!(a.role & 0x01),
        })),
        ...extraAccounts
    ];

    const txIx = new TransactionInstruction({
        programId: new PublicKey(ix.programAddress),
        keys,
        data: Buffer.from(ix.data),
    });

    const result = await processTransaction(context, [txIx], signers);
    if (result.result) {
        throw new Error(`Execution failed: ${result.result}`);
    }
    return result;
}

export async function tryProcessInstruction(context: any, ix: any, signers: Keypair[] = [], extraAccounts: any[] = []) {
    const keys = [
        ...ix.accounts.map((a: any) => ({
            pubkey: new PublicKey(a.address),
            isSigner: !!(a.role & 0x02),
            isWritable: !!(a.role & 0x01),
        })),
        ...extraAccounts
    ];

    const txIx = new TransactionInstruction({
        programId: new PublicKey(ix.programAddress),
        keys,
        data: Buffer.from(ix.data),
    });

    const tx = new Transaction();
    tx.recentBlockhash = (await context.banksClient.getLatestBlockhash())[0];
    tx.feePayer = context.payer.publicKey;
    tx.add(txIx);
    // Add dummy transfer to make transaction unique and avoid "Already Processed" replay error
    tx.add(SystemProgram.transfer({
        fromPubkey: context.payer.publicKey,
        toPubkey: context.payer.publicKey,
        lamports: 0,
    }));
    tx.sign(context.payer, ...signers);

    return await context.banksClient.tryProcessTransaction(tx);
}
