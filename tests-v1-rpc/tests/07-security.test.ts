/**
 * 07-security.test.ts
 *
 * Tests for: Security checklist gaps + Audit regression suite
 * Merged from: security_checklist.test.ts + audit_regression.test.ts
 */

import { expect, describe, it, beforeAll } from "vitest";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { setupTest, sendTx, getRandomSeed, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  AuthType,
} from "@lazorkit/solita-client";

describe("Security & Audit Regression", () => {
  let ctx: TestContext;
  let walletPda: PublicKey;
  let vaultPda: PublicKey;
  let owner: Keypair;
  let ownerAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();
    owner = Keypair.generate();

    const { ix: ixCreate, walletPda: w, authorityPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
    });
    await sendTx(ctx, [ixCreate]);
    walletPda = w;
    ownerAuthPda = authorityPda;

    const [v] = findVaultPda(walletPda);
    vaultPda = v;

    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPda, 100_000_000n)]);
  }, 30_000);

  // ─── Security Checklist ────────────────────────────────────────────────────

  it("CreateSession rejects System Program spoofing", async () => {
    const sessionKey = Keypair.generate();

    const { ix } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      sessionKey: sessionKey.publicKey,
      expiresAt: 999999999n,
      walletPda
    });

    const spoofedSystemProgram = Keypair.generate().publicKey;
    ix.keys = ix.keys.map((k: any, i: number) =>
      i === 4 ? { ...k, pubkey: spoofedSystemProgram } : k
    );

    const result = await tryProcessInstructions(ctx, [ix], [ctx.payer, owner]);
    expect(result.result).not.toBe("ok");
  });

  it("CloseSession: protocol admin cannot close an active session without wallet auth", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const { ix: ixCreateSession } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      sessionKey: sessionKey.publicKey,
      expiresAt: BigInt(2 ** 62),
      walletPda
    });
    await sendTx(ctx, [ixCreateSession], [owner]);

    const closeIx = await ctx.highClient.closeSession({
      payer: ctx.payer,
      walletPda,
      sessionPda,
    });

    const result = await tryProcessInstructions(ctx, [closeIx], [ctx.payer]);
    expect(result.result).not.toBe("ok");
  });

  // ─── Audit Regression ─────────────────────────────────────────────────────

  it("Regression: SweepTreasury preserves rent-exemption and remains operational", async () => {
    const pubkeyBytes = ctx.payer.publicKey.toBytes();
    const sum = pubkeyBytes.reduce((a: number, b: number) => a + b, 0);
    const shardId = sum % 16;

    const sweepIx = await ctx.highClient.sweepTreasury({
      admin: ctx.payer,
      destination: ctx.payer.publicKey,
      shardId,
    });
    await sendTx(ctx, [sweepIx]);

    const shardInfo = await ctx.connection.getAccountInfo(ctx.treasuryShard);
    const RENT_EXEMPT_MIN = await ctx.connection.getMinimumBalanceForRentExemption(shardInfo!.data.length);
    const postSweepBalance = await ctx.connection.getBalance(ctx.treasuryShard);
    expect(postSweepBalance).toBeGreaterThanOrEqual(RENT_EXEMPT_MIN);

    // Operationality Check
    const recipient = Keypair.generate().publicKey;
    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: ownerAuthPda,
      innerInstructions: [getSystemTransferIx(vaultPda, recipient, BigInt(RENT_EXEMPT_MIN))],
      signer: owner
    });
    await sendTx(ctx, [executeIx], [owner]);

    const configInfo = await ctx.connection.getAccountInfo(ctx.configPda);
    const actionFee = configInfo!.data.readBigUInt64LE(48);

    const finalBalance = await ctx.connection.getBalance(ctx.treasuryShard);
    expect(finalBalance).toBeGreaterThanOrEqual(postSweepBalance + Number(actionFee));
  });

  it("Regression: CloseWallet rejects self-transfer to prevent burn", async () => {
    const closeIx = await ctx.highClient.closeWallet({
      payer: ctx.payer,
      walletPda,
      destination: vaultPda,
      adminType: AuthType.Ed25519,
      adminSigner: owner
    });

    const result = await tryProcessInstructions(ctx, [closeIx], [ctx.payer, owner]);
    expect(result.result).not.toBe("ok");
  });

  it("Regression: CloseSession rejects Config PDA spoofing", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const { ix: ixCreateSession } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      sessionKey: sessionKey.publicKey,
      expiresAt: BigInt(2 ** 62),
      walletPda
    });
    await sendTx(ctx, [ixCreateSession], [owner]);

    const [fakeConfigPda] = await PublicKey.findProgramAddress(
      [Buffer.from("fake_config")],
      ctx.payer.publicKey
    );

    const closeSessionIx = await ctx.highClient.closeSession({
      payer: ctx.payer,
      walletPda,
      sessionPda,
      configPda: fakeConfigPda,
      authorizer: { authorizerPda: ownerAuthPda, signer: owner }
    });

    const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer, owner]);
    expect(result.result).not.toBe("ok");
  });

  it("Regression: No protocol fees on cleanup instructions", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const { ix: ixCreateSession } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      sessionKey: sessionKey.publicKey,
      expiresAt: BigInt(2 ** 62),
      walletPda
    });
    await sendTx(ctx, [ixCreateSession], [owner]);

    const payerBalanceBefore = await ctx.connection.getBalance(ctx.payer.publicKey);

    const closeSessionIx = await ctx.highClient.closeSession({
      payer: ctx.payer,
      walletPda,
      sessionPda,
      authorizer: { authorizerPda: ownerAuthPda, signer: owner }
    });
    // This transaction shouldn't charge the 1000 lamports action fee
    const txId1 = await sendTx(ctx, [closeSessionIx], [owner]);

    const closeWalletIx = await ctx.highClient.closeWallet({
      payer: ctx.payer,
      walletPda,
      destination: ctx.payer.publicKey,
      adminType: AuthType.Ed25519,
      adminSigner: owner
    });
    // This transaction shouldn't charge the 10000 lamports action fee
    const txId2 = await sendTx(ctx, [closeWalletIx], [owner]);

    const payerBalanceAfter = await ctx.connection.getBalance(ctx.payer.publicKey);
    
    // The only cost should be the 2 transaction signature fees (usually 5000 lamports each)
    // plus potential rent refunds. We can just verify the payer didn't lose more than network fees.
    const expectedMaxCost = 15000;
    expect(payerBalanceBefore - payerBalanceAfter).toBeLessThan(expectedMaxCost);
  });
});
