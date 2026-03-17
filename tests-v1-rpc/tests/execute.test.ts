/**
 * LazorKit V1 Client — Execute tests
 *
 * Tests: Execute instruction with SOL transfer inner instruction.
 */

import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  packCompactInstructions,
  type CompactInstruction,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, PROGRAM_ID, type TestContext } from "./common";

describe("LazorKit V1 — Execute", () => {
  let ctx: TestContext;

  let ownerKeypair: Keypair;
  let walletPda: PublicKey;
  let vaultPda: PublicKey;
  let ownerAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();

    // Create a wallet — only payer signs
    ownerKeypair = Keypair.generate();
    const userSeed = new Uint8Array(32);
    crypto.getRandomValues(userSeed);

    [walletPda] = findWalletPda(userSeed);
    [vaultPda] = findVaultPda(walletPda);
    [ownerAuthPda] = findAuthorityPda(
      walletPda,
      ownerKeypair.publicKey.toBytes()
    );

    const createWalletIx = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: ownerAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: ownerKeypair.publicKey.toBytes(),
    });

    await sendTx(ctx, [createWalletIx]);
    console.log("Wallet created for execute tests");

    // Fund the vault so it has SOL to transfer
    const fundTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: ctx.payer.publicKey,
        toPubkey: vaultPda,
        lamports: 0.1 * LAMPORTS_PER_SOL,
      })
    );
    await sendAndConfirmTransaction(ctx.connection, fundTx, [ctx.payer], {
      commitment: "confirmed",
    });
    console.log("Vault funded with 0.1 SOL");
  }, 30_000);

  it("should execute a SOL transfer from vault", async () => {
    const recipient = Keypair.generate().publicKey;

    // Build inner SystemProgram.transfer instruction in compact format
    // Account indexes in the Execute instruction:
    // 0: payer, 1: wallet, 2: authority, 3: vault, 4: config, 5: treasuryShard, 6: systemProgram, 7: sysvarInstructions
    // We need: vault (3) as source, recipient as new account

    const transferAmount = 10_000; // 0.00001 SOL

    // SystemProgram.Transfer instruction data: [2,0,0,0] + u64 LE amount
    const transferData = Buffer.alloc(12);
    transferData.writeUInt32LE(2, 0); // Transfer instruction index
    transferData.writeBigUInt64LE(BigInt(transferAmount), 4);

    // recipient will be at index 8 (after sysvarInstructions at 7)
    // But we also add the authorizerSigner at the end. Let's see the account layout:
    // 0: payer, 1: wallet, 2: authority, 3: vault, 4: config, 5: treasuryShard,
    // 6: systemProgram, 7: sysvarInstructions (= programId placeholder),
    // remaining: [recipient(8)], authorizerSigner(9)

    const compactIxs: CompactInstruction[] = [
      {
        programIdIndex: 6,  // SystemProgram
        accountIndexes: [3, 8], // vault=3, recipient=8 (remaining account)
        data: transferData,
      },
    ];

    const packedInstructions = packCompactInstructions(compactIxs);

    const executeIx = ctx.client.execute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: ownerAuthPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      packedInstructions,
      authorizerSigner: ownerKeypair.publicKey,
      remainingAccounts: [
        { pubkey: recipient, isWritable: true, isSigner: false },
      ],
    });

    const recipientBalanceBefore = await ctx.connection.getBalance(recipient);
    expect(recipientBalanceBefore).toBe(0);

    const sig = await sendTx(ctx, [executeIx], [ownerKeypair]);
    expect(sig).toBeDefined();
    console.log("Execute signature:", sig);

    // Verify the recipient received the SOL
    const recipientBalanceAfter = await ctx.connection.getBalance(recipient);
    expect(recipientBalanceAfter).toBe(transferAmount);
  }, 30_000);
});
