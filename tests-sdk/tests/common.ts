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

export const PROGRAM_ID = new PublicKey('FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao');
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
