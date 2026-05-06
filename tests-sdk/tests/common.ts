import * as fs from 'fs';
import * as path from 'path';
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  sendAndConfirmTransaction,
  Transaction,
  TransactionInstruction,
  type Signer,
} from '@solana/web3.js';

/**
 * Resolve the program ID the test suite should target. Order:
 *
 *   1. `PROGRAM_ID` env var (CI / explicit override)
 *   2. The pubkey of `target/deploy/lazorkit_program-keypair.json` — this is
 *      the address `solana-test-validator --bpf-program $(solana-keygen pubkey ...)`
 *      loaded the binary at, so PDA derivations on the client side match
 *      what the program sees at runtime.
 *   3. Foundation devnet fallback (`FLb7…`) — used when neither env nor
 *      keypair file exist. Won't actually work end-to-end without a real
 *      validator setup, but lets type-check / static tooling proceed.
 *
 * Prior to this, PROGRAM_ID was hardcoded to FLb7 — broken for any locally
 * built binary because cargo build-sbf generates a fresh keypair on first
 * build (unless one is already present at the target path).
 */
function loadProgramId(): PublicKey {
  if (process.env.PROGRAM_ID) {
    return new PublicKey(process.env.PROGRAM_ID);
  }
  const keypairPath = path.resolve(
    __dirname,
    '../../target/deploy/lazorkit_program-keypair.json',
  );
  if (fs.existsSync(keypairPath)) {
    const secret = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
    return Keypair.fromSecretKey(new Uint8Array(secret)).publicKey;
  }
  return new PublicKey('FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao');
}

export const PROGRAM_ID = loadProgramId();
export const RPC_URL = process.env.RPC_URL || 'http://127.0.0.1:8899';

export interface TestContext {
  connection: Connection;
  payer: Keypair;
}

export async function setupTest(): Promise<TestContext> {
  const connection = new Connection(RPC_URL, 'confirmed');
  const payer = Keypair.generate();

  const sig = await connection.requestAirdrop(payer.publicKey, 10 * LAMPORTS_PER_SOL);
  await connection.confirmTransaction(sig, 'confirmed');

  return { connection, payer };
}

export async function sendTx(
  ctx: TestContext,
  instructions: TransactionInstruction[],
  signers: Signer[] = [],
): Promise<string> {
  const tx = new Transaction();
  for (const ix of instructions) tx.add(ix);
  return sendAndConfirmTransaction(ctx.connection, tx, [ctx.payer, ...signers], {
    commitment: 'confirmed',
  });
}

export async function sendTxExpectError(
  ctx: TestContext,
  instructions: TransactionInstruction[],
  signers: Signer[] = [],
  expectedErrorCode?: number,
): Promise<string> {
  try {
    const tx = new Transaction();
    for (const ix of instructions) tx.add(ix);
    await sendAndConfirmTransaction(ctx.connection, tx, [ctx.payer, ...signers], {
      commitment: 'confirmed',
    });
    throw new Error('Transaction should have failed but succeeded');
  } catch (err: any) {
    const msg = String(err);
    if (msg.includes('Transaction should have failed')) throw err;
    if (expectedErrorCode !== undefined) {
      const hexCode = expectedErrorCode.toString(16);
      if (!msg.includes(`0x${hexCode}`) && !msg.includes(`Custom(${expectedErrorCode})`)) {
        throw new Error(
          `Expected error code ${expectedErrorCode} (0x${hexCode}), got: ${msg}`,
        );
      }
    }
    return msg;
  }
}

export async function getSlot(ctx: TestContext): Promise<bigint> {
  const slot = await ctx.connection.getSlot('confirmed');
  // Use current slot directly; Clock::get() validates slot age (< 150 slots).
  return BigInt(slot);
}
