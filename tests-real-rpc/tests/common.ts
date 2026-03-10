import {
    createSolanaRpc,
    address,
    type Address,
    type TransactionSigner,
    type Instruction,
    generateKeyPairSigner,
    pipe,
    createTransactionMessage,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    appendTransactionMessageInstruction,
    addSignersToTransactionMessage,
    compileTransaction,
    signTransactionMessageWithSigners,
    getBase64EncodedWireTransaction,
    sendAndConfirmTransactionFactory,
    getSignatureFromTransaction,
    createSolanaRpcSubscriptions,
    lamports,
} from "@solana/kit";
import { LazorClient } from "../../sdk/lazorkit-ts/src";
import * as dotenv from "dotenv";
import bs58 from "bs58";
import { createKeyPairSignerFromBytes } from "@solana/kit";

dotenv.config();

const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

export const PROGRAM_ID_STR = "2m47smrvCRpuqAyX2dLqPxpAC1658n1BAQga1wRCsQiT";

export interface TestContext {
    rpc: any;
    rpcSubscriptions: any;
    payer: TransactionSigner;
    configPda: Address;
    treasuryShard: Address;
}

export async function setupTest(): Promise<{ context: TestContext, client: LazorClient }> {
    const rpcUrl = process.env.RPC_URL || "http://127.0.0.1:8899";
    const wsUrl = process.env.WS_URL || "ws://127.0.0.1:8900";
    const rpc = createSolanaRpc(rpcUrl);
    const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl);

    let payer: TransactionSigner;
    let skipAirdrop = false;

    if (process.env.PRIVATE_KEY) {
        let keyBytes: Uint8Array;
        if (process.env.PRIVATE_KEY.startsWith('[')) {
            keyBytes = new Uint8Array(JSON.parse(process.env.PRIVATE_KEY));
        } else {
            keyBytes = bs58.decode(process.env.PRIVATE_KEY);
        }
        payer = await createKeyPairSignerFromBytes(keyBytes);
        skipAirdrop = true; // Use fixed account, usually already has funds
        console.log(`Using fixed payer: ${payer.address}`);
    } else {
        payer = await generateKeyPairSigner();
    }

    // Check balance and log it
    try {
        const balance = await rpc.getBalance(payer.address).send();
        console.log(`Payer balance: ${Number(balance.value) / 1e9} SOL`);

        // If balance is low (< 0.5 SOL), try airdrop anyway (if not on mainnet)
        if (balance.value < 500_000_000n && !rpcUrl.includes("mainnet")) {
            console.log("Balance low. Attempting airdrop...");
            await rpc.requestAirdrop(payer.address, lamports(1_000_000_000n)).send();
            await sleep(2000);
            const newBalance = await rpc.getBalance(payer.address).send();
            console.log(`New balance: ${Number(newBalance.value) / 1e9} SOL`);
        }
    } catch (e) {
        console.warn("Could not check balance or airdrop.");
    }

    const client = new LazorClient(rpc);

    // Compute Config and Treasury Shard PDAs
    const { findConfigPda, findTreasuryShardPda } = await import("../../sdk/lazorkit-ts/src/utils/pdas");
    const { getAddressEncoder } = await import("@solana/kit");

    const [configPda] = await findConfigPda();
    const pubkeyBytes = getAddressEncoder().encode(payer.address);
    const sum = pubkeyBytes.reduce((a, b) => a + b, 0);
    const shardId = sum % 16;
    const [treasuryShard] = await findTreasuryShardPda(shardId);

    // Initialize Config if not exists
    try {
        const accInfo = await rpc.getAccountInfo(configPda, { commitment: "processed" }).send();
        if (!accInfo || !accInfo.value) throw new Error("Not initialized");
    } catch {
        console.log("Initializing Global Config and Treasury Shard...");
        const { getInitializeConfigInstruction, getInitTreasuryShardInstruction } = await import("../../sdk/lazorkit-ts/src");

        const initConfigIx = getInitializeConfigInstruction({
            admin: payer,
            config: configPda,
            systemProgram: "11111111111111111111111111111111" as Address,
            rent: "SysvarRent111111111111111111111111111111111" as Address,
            walletFee: 10000n, // 0.00001 SOL
            actionFee: 1000n,  // 0.000001 SOL
            numShards: 16
        });

        const initShardIx = getInitTreasuryShardInstruction({
            payer: payer,
            config: configPda,
            treasuryShard: treasuryShard,
            systemProgram: "11111111111111111111111111111111" as Address,
            rent: "SysvarRent111111111111111111111111111111111" as Address,
            shardId,
        });

        try {
            await processInstructions({ rpc, rpcSubscriptions, payer, configPda, treasuryShard }, [initConfigIx, initShardIx], [payer]);
        } catch (e: any) {
            // Ignore if already initialized by another parallel/sequential test
            console.warn("Config initialization skipped or failed:", e.message);
        }
    }

    // Initialize Treasury Shard if not exists (in case config existed but not this shard)
    try {
        const accInfo = await rpc.getAccountInfo(treasuryShard, { commitment: "processed" }).send();
        if (!accInfo || !accInfo.value) throw new Error("Not initialized");
    } catch {
        console.log(`Initializing Treasury Shard ${shardId}...`);
        const { getInitTreasuryShardInstruction } = await import("../../sdk/lazorkit-ts/src");
        const initShardIx = getInitTreasuryShardInstruction({
            payer: payer,
            config: configPda,
            treasuryShard: treasuryShard,
            systemProgram: "11111111111111111111111111111111" as Address,
            rent: "SysvarRent111111111111111111111111111111111" as Address,
            shardId,
        });

        try {
            await processInstructions({ rpc, rpcSubscriptions, payer, configPda, treasuryShard }, [initShardIx], [payer]);
        } catch (e: any) {
            console.warn(`Shard ${shardId} initialization skipped or failed:`, e.message);
        }
    }

    return {
        context: { rpc, rpcSubscriptions, payer, configPda, treasuryShard },
        client
    };
}

export async function processInstruction(context: TestContext, ix: any, signers: TransactionSigner[] = [], extraAccounts: any[] = []) {
    const { rpc, rpcSubscriptions, payer } = context;

    let retries = 0;
    const maxRetries = 3;

    while (retries < maxRetries) {
        try {
            // Add a small delay for Devnet to avoid 429
            if (process.env.RPC_URL?.includes("devnet")) {
                await sleep(1000 + (retries * 2000)); // Exponential backoff
            }

            const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

            const accounts = [...(ix.accounts || [])];
            console.log("Execute accounts order:", accounts.map(a => a.address));
            for (const acc of extraAccounts) {
                accounts.push(acc);
            }

            const transactionMessage = pipe(
                createTransactionMessage({ version: 0 }),
                m => setTransactionMessageFeePayerSigner(payer, m),
                m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
                m => appendTransactionMessageInstruction({
                    ...ix,
                    accounts
                } as Instruction, m),
                m => addSignersToTransactionMessage(signers, m)
            );

            const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
            const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

            await sendAndConfirm(signedTransaction as any, {
                commitment: 'confirmed',
            });

            return getSignatureFromTransaction(signedTransaction);

        } catch (e: any) {
            const isRateLimit = e.message?.includes("429") ||
                e.context?.headers?.status === 429 ||
                e.context?.status === 429;
            if (isRateLimit && retries < maxRetries - 1) {
                retries++;
                console.log(`Rate limited (429). Retrying ${retries}/${maxRetries}...`);
                continue;
            }

            if (e.context?.logs) {
                console.error("Simulation Logs:\n", e.context.logs.join("\n"));
            }
            throw e;
        }
    }
    throw new Error("Max retries exceeded for transaction");
}

export async function tryProcessInstructions(context: TestContext, ixs: any[], signers: TransactionSigner[] = []) {
    try {
        const signature = await processInstructions(context, ixs, signers);
        return { result: "ok", signature };
    } catch (e: any) {
        console.error("DEBUG: Instructions failed:", e);

        let result = e.message || "Unknown Error";

        // Extract error code if available (Solana v2 style)
        const code = e.context?.code || e.cause?.context?.code || e.data?.code;
        if (code !== undefined) {
            result += ` (Code: ${code})`;
        }

        // Include logs which often contain the actual program error message
        const logs = e.context?.logs || e.cause?.context?.logs || e.data?.logs || [];
        if (logs.length > 0) {
            result += " | LOGS: " + logs.join("\n");
        }

        return { result };
    }
}

export async function processInstructions(context: TestContext, ixs: any[], signers: TransactionSigner[] = []) {
    const { rpc, rpcSubscriptions, payer } = context;

    let retries = 0;
    const maxRetries = 3;

    while (retries < maxRetries) {
        try {
            if (process.env.RPC_URL?.includes("devnet")) {
                await sleep(1000 + (retries * 2000));
            }

            const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

            let transactionMessage = pipe(
                createTransactionMessage({ version: 0 }),
                m => setTransactionMessageFeePayerSigner(payer, m),
                m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m)
            );

            for (const ix of ixs) {
                transactionMessage = appendTransactionMessageInstruction(ix as any, transactionMessage as any) as any;
            }

            transactionMessage = addSignersToTransactionMessage(signers, transactionMessage);

            const signedTransaction = await signTransactionMessageWithSigners(transactionMessage as any);
            const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

            await sendAndConfirm(signedTransaction as any, {
                commitment: 'confirmed',
            });

            return getSignatureFromTransaction(signedTransaction);

        } catch (e: any) {
            const isRateLimit = e.message?.includes("429") ||
                e.context?.headers?.status === 429 ||
                e.context?.status === 429;
            if (isRateLimit && retries < maxRetries - 1) {
                retries++;
                console.log(`Rate limited (429). Retrying ${retries}/${maxRetries}...`);
                continue;
            }

            if (e.context?.logs) {
                console.error("Simulation Logs:\n", e.context.logs.join("\n"));
            }
            throw e;
        }
    }
    throw new Error("Max retries reached");
}

export async function tryProcessInstruction(context: TestContext, ix: any, signers: TransactionSigner[] = []) {
    try {
        const signature = await processInstruction(context, ix, signers);
        return { result: "ok", signature };
    } catch (e: any) {
        console.error("DEBUG: Instruction failed:", e);

        let result = e.message || "Unknown Error";

        // Extract error code if available (Solana v2 style)
        const code = e.context?.code || e.cause?.context?.code || e.data?.code;
        if (code !== undefined) {
            result += ` (Code: ${code})`;
        }

        // Include logs which often contain the actual program error message
        const logs = e.context?.logs || e.cause?.context?.logs || e.data?.logs || [];
        if (logs.length > 0) {
            result += " | LOGS: " + logs.join("\n");
        }

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
