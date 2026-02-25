import {
    createSolanaRpc,
    address,
    type Address,
    type TransactionSigner,
    type Instruction,
    generateKeyPairSigner,
    pipe,
    createTransactionMessage,
    setTransactionMessageFeePayer,
    setTransactionMessageLifetimeUsingBlockhash,
    appendTransactionMessageInstruction,
    compileTransaction,
    signTransactionMessageWithSigners,
    getBase64EncodedWireTransaction,
    sendAndConfirmTransactionFactory,
    getSignatureFromTransaction,
    createSolanaRpcSubscriptions,
    lamports,
} from "@solana/kit";
import { LazorClient } from "../../sdk/lazorkit-ts/src";

export const PROGRAM_ID_STR = "Btg4mLUdMd3ov8PBtmuuFMAimLAdXyew9XmsGtuY9VcP";

export interface TestContext {
    rpc: any;
    rpcSubscriptions: any;
    payer: TransactionSigner;
}

export async function setupTest(): Promise<{ context: TestContext, client: LazorClient }> {
    const rpc = createSolanaRpc("http://127.0.0.1:8899");
    const rpcSubscriptions = createSolanaRpcSubscriptions("ws://127.0.0.1:8900");
    const payer = await generateKeyPairSigner();

    // Airdrop to payer
    try {
        const airdropResult = await rpc.requestAirdrop(payer.address, lamports(2_000_000_000n)).send();
        // Simple delay for airdrop confirmation in localnet
        await new Promise(resolve => setTimeout(resolve, 500));
    } catch (e) {
        console.warn("Airdrop failed, if you are running against a persistent localnet this might be fine if payer has funds or if airdrop is disabled.");
    }

    const client = new LazorClient(rpc);

    return {
        context: { rpc, rpcSubscriptions, payer },
        client
    };
}

export async function processInstruction(context: TestContext, ix: any, signers: TransactionSigner[] = [], extraAccounts: any[] = []) {
    const { rpc, rpcSubscriptions, payer } = context;

    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

    const accounts = [...(ix.accounts || [])];
    if (extraAccounts.length > 0) {
        accounts.push(...extraAccounts);
    }

    const transactionMessage = pipe(
        createTransactionMessage({ version: 0 }),
        m => setTransactionMessageFeePayer(payer.address, m),
        m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
        m => appendTransactionMessageInstruction({
            ...ix,
            accounts
        } as Instruction, m)
    );

    const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);

    const wireTransaction = getBase64EncodedWireTransaction(signedTransaction);

    const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

    await sendAndConfirm(signedTransaction as any, { commitment: 'confirmed' });

    return getSignatureFromTransaction(signedTransaction);
}

export async function tryProcessInstruction(context: TestContext, ix: any, signers: TransactionSigner[] = []) {
    try {
        const signature = await processInstruction(context, ix, signers);
        return { result: "ok", signature };
    } catch (e: any) {
        console.error("DEBUG: Instruction failed:", e);
        // Return error message for assertions, including logs if available
        let result = e.message || JSON.stringify(e);
        if (e.context?.logs) result += " | " + e.context.logs.join("\n");
        if (e.data?.logs) result += " | " + e.data.logs.join("\n");
        return { result };
    }
}

export function getSystemTransferIx(from: TransactionSigner | Address, to: Address, amount: bigint) {
    const fromAddress = typeof from === 'string' ? from : from.address;
    const fromSigner = typeof from === 'string' ? undefined : from;
    const data = new Uint8Array(12);
    data[0] = 2; // Transfer
    const view = new DataView(data.buffer);
    view.setBigUint64(4, amount, true);
    return {
        programAddress: "11111111111111111111111111111111" as Address,
        accounts: [
            { address: fromAddress, role: 3, ...(fromSigner ? { signer: fromSigner } : {}) },
            { address: to, role: 1 },
        ],
        data
    };
}
