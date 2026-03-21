/**
 * Shared test setup for LazorKit V1 (web3.js) tests.
 *
 * - Connection + payer with airdrop
 * - Config PDA + Treasury Shard initialization
 * - sendTx() helper
 */

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  type TransactionInstruction,
} from "@solana/web3.js";
import {
  findConfigPda,
  findTreasuryShardPda,
  LazorClient,
  PROGRAM_ID,
} from "@lazorkit/solita-client";

export { PROGRAM_ID };
import * as dotenv from "dotenv";

dotenv.config();

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Generates a random 32-byte seed. Shared across all test files. */
export function getRandomSeed(): Uint8Array {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  return seed;
}

export interface TestContext {
  connection: Connection;
  payer: Keypair;
  configPda: PublicKey;
  treasuryShard: PublicKey;
  shardId: number;
  highClient: LazorClient;
}

/**
 * Send one or more instructions wrapped in a single Transaction.
 */
export async function sendTx(
  ctx: TestContext,
  instructions: TransactionInstruction[],
  signers: Keypair[] = []
): Promise<string> {
  const tx = new Transaction();
  for (const ix of instructions) {
    tx.add(ix);
  }
  const allSigners = [ctx.payer, ...signers];
  return sendAndConfirmTransaction(ctx.connection, tx, allSigners, {
    commitment: "confirmed",
  });
}

/**
 * Send instructions expecting a failure, returning error string for matching.
 */
export async function tryProcessInstruction(
  ctx: TestContext,
  instructions: import("@solana/web3.js").TransactionInstruction | import("@solana/web3.js").TransactionInstruction[],
  signers: Keypair[] = []
): Promise<{ result: string }> {
  try {
    const ixs = Array.isArray(instructions) ? instructions : [instructions];
    await sendTx(ctx, ixs, signers);
    return { result: "ok" };
  } catch (e: any) {
    return { result: e.message || "simulation failed" };
  }
}

/**
 * Multiple instructions variant
 */
export async function tryProcessInstructions(
  ctx: TestContext,
  instructions: TransactionInstruction[],
  signers: Keypair[] = []
): Promise<{ result: string }> {
  try {
    await sendTx(ctx, instructions, signers);
    return { result: "ok" };
  } catch (e: any) {
    return { result: e.message || "simulation failed" };
  }
}

/**
 * Initialize test context:
 * - Create connection
 * - Generate or load payer and airdrop
 * - Derive and initialize Config PDA
 * - Derive and initialize Treasury Shard PDA
 */
export async function setupTest(): Promise<TestContext> {
  const rpcUrl = process.env.RPC_URL || "http://127.0.0.1:8899";
  const connection = new Connection(rpcUrl, "confirmed");

  // ── Payer ─────────────────────────────────────────────────────────
  let payer: Keypair;
  if (process.env.PRIVATE_KEY) {
    let keyBytes: Uint8Array;
    if (process.env.PRIVATE_KEY.startsWith("[")) {
      keyBytes = new Uint8Array(JSON.parse(process.env.PRIVATE_KEY));
    } else {
      // base58
      const bs58 = await import("bs58");
      keyBytes = bs58.default.decode(process.env.PRIVATE_KEY);
    }
    payer = Keypair.fromSecretKey(keyBytes);
    console.log(`Using fixed payer: ${payer.publicKey.toBase58()}`);
  } else {
    payer = Keypair.generate();
  }

  // Airdrop if needed
  try {
    const balance = await connection.getBalance(payer.publicKey);
    console.log(`Payer balance: ${balance / 1e9} SOL`);

    if (balance < 500_000_000 && !rpcUrl.includes("mainnet")) {
      console.log("Balance low, requesting airdrop...");
      const sig = await connection.requestAirdrop(
        payer.publicKey,
        2_000_000_000
      );
      const latestBlockHash = await connection.getLatestBlockhash();
      await connection.confirmTransaction({
        blockhash: latestBlockHash.blockhash,
        lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
        signature: sig,
      });
      await sleep(1000);
      console.log(
        `New balance: ${(await connection.getBalance(payer.publicKey)) / 1e9} SOL`
      );
    }
  } catch (e) {
    console.warn("Could not check balance or airdrop:", e);
  }

  // ── Client ────────────────────────────────────────────────────────
  const highClient = new LazorClient(connection, PROGRAM_ID);

  // ── Config PDA ────────────────────────────────────────────────────
  const [configPda] = findConfigPda(PROGRAM_ID);

  // ── Treasury Shard ────────────────────────────────────────────────
  const pubkeyBytes = payer.publicKey.toBytes();
  const sum = pubkeyBytes.reduce((a, b) => a + b, 0);
  const shardId = sum % 16;
  const [treasuryShard] = findTreasuryShardPda(shardId, PROGRAM_ID);

  const ctx: TestContext = {
    connection,
    payer,
    configPda,
    treasuryShard,
    shardId,
    highClient,
  };

  // ── Initialize Config if not yet ──────────────────────────────────
  try {
    const accInfo = await connection.getAccountInfo(configPda);
    if (!accInfo) throw new Error("Not initialized");
  } catch {
    console.log("Initializing Global Config...");
    try {
      const initConfigIx = await highClient.initializeConfig({
        admin: payer,
        walletFee: 10000n,
        actionFee: 1000n,
        numShards: 16,
      });
      await sendTx(ctx, [initConfigIx]);
    } catch (e: any) {
      console.warn("Config init skipped:", e.message);
    }
  }

  // ── Initialize Treasury Shard if not yet ──────────────────────────
  try {
    const accInfo = await connection.getAccountInfo(treasuryShard);
    if (!accInfo) throw new Error("Not initialized");
  } catch {
    console.log(`Initializing Treasury Shard ${shardId}...`);
    try {
      const initShardIx = await highClient.initTreasuryShard({
        payer: payer,
        shardId,
      });
      await sendTx(ctx, [initShardIx]);
    } catch (e: any) {
      console.warn(`Shard ${shardId} init skipped:`, e.message);
    }
  }

  return ctx;
}

export function getSystemTransferIx(
  fromPubkey: PublicKey,
  toPubkey: PublicKey,
  lamports: bigint
) {
  return SystemProgram.transfer({
    fromPubkey,
    toPubkey,
    lamports: Number(lamports),
  });
}
