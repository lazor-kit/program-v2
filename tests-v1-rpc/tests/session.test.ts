/**
 * LazorKit V1 Client — Session tests
 *
 * Tests: CreateSession, CloseSession.
 */

import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, type TestContext } from "./common";

describe("LazorKit V1 — Session", () => {
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
    console.log("Wallet created for session tests");
  }, 30_000);

  it("should create a session", async () => {
    const sessionKeypair = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKeypair.publicKey);

    // Expire in 1 hour
    const expiresAt = BigInt(Math.floor(Date.now() / 1000) + 3600);

    const ix = ctx.client.createSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      session: sessionPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      sessionKey: Array.from(sessionKeypair.publicKey.toBytes()),
      expiresAt,
      authorizerSigner: ownerKeypair.publicKey,
    });

    console.log("CreateSession Data Length:", ix.data.length);
    console.log("CreateSession Data (hex):", Buffer.from(ix.data).toString('hex'));

    const sig = await sendTx(ctx, [ix], [ownerKeypair]);
    expect(sig).toBeDefined();
    console.log("CreateSession signature:", sig);

    // Verify session account exists
    const sessionAccount = await ctx.connection.getAccountInfo(sessionPda);
    expect(sessionAccount).not.toBeNull();
  }, 30_000);

  it("should create and close a session", async () => {
    const sessionKeypair = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKeypair.publicKey);

    const expiresAt = BigInt(Math.floor(Date.now() / 1000) + 3600);

    // Create
    const createIx = ctx.client.createSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      session: sessionPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      sessionKey: Array.from(sessionKeypair.publicKey.toBytes()),
      expiresAt,
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [createIx], [ownerKeypair]);

    // Verify it exists
    let sessionAccount = await ctx.connection.getAccountInfo(sessionPda);
    expect(sessionAccount).not.toBeNull();

    // Close
    const closeIx = ctx.client.closeSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      session: sessionPda,
      config: ctx.configPda,
      authorizer: ownerAuthPda,
      authorizerSigner: ownerKeypair.publicKey,
    });

    const sig = await sendTx(ctx, [closeIx], [ownerKeypair]);
    expect(sig).toBeDefined();
    console.log("CloseSession signature:", sig);

    // Verify session is closed
    sessionAccount = await ctx.connection.getAccountInfo(sessionPda);
    expect(sessionAccount).toBeNull();
  }, 30_000);
});
