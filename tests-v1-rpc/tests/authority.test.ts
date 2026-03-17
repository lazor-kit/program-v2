/**
 * LazorKit V1 Client — Authority tests
 *
 * Tests: AddAuthority, RemoveAuthority with Ed25519 keys.
 */

import { Keypair } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, type TestContext } from "./common";

describe("LazorKit V1 — Authority", () => {
  let ctx: TestContext;

  // Shared state across tests in this suite
  let ownerKeypair: Keypair;
  let userSeed: Uint8Array;
  let walletPda: import("@solana/web3.js").PublicKey;
  let vaultPda: import("@solana/web3.js").PublicKey;
  let ownerAuthPda: import("@solana/web3.js").PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();

    // Create a wallet first for authority tests
    ownerKeypair = Keypair.generate();
    userSeed = new Uint8Array(32);
    crypto.getRandomValues(userSeed);

    [walletPda] = findWalletPda(userSeed);
    [vaultPda] = findVaultPda(walletPda);
    [ownerAuthPda] = findAuthorityPda(
      walletPda,
      ownerKeypair.publicKey.toBytes()
    );

    // CreateWallet — only payer signs
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
    console.log("Wallet created for authority tests");
  }, 30_000);

  it("should add a new Ed25519 authority (Writer role)", async () => {
    const newAuthorizer = Keypair.generate();
    const [newAuthPda] = findAuthorityPda(
      walletPda,
      newAuthorizer.publicKey.toBytes()
    );

    // AddAuthority requires the ownerKeypair to sign as authorizerSigner
    const ix = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: newAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, // Ed25519
      newRole: 1,  // Writer
      authPubkey: newAuthorizer.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    });

    const sig = await sendTx(ctx, [ix], [ownerKeypair]);
    expect(sig).toBeDefined();
    console.log("AddAuthority signature:", sig);

    // Verify it exists
    const authAccountInfo = await ctx.connection.getAccountInfo(newAuthPda);
    expect(authAccountInfo).not.toBeNull();
  }, 30_000);

  it("should remove an authority", async () => {
    // First, add a new authority to remove
    const tempAuthorizer = Keypair.generate();
    const [tempAuthPda] = findAuthorityPda(
      walletPda,
      tempAuthorizer.publicKey.toBytes()
    );

    const addIx = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: tempAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      newRole: 1,
      authPubkey: tempAuthorizer.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [addIx], [ownerKeypair]);

    // Verify it exists
    let authAccountInfo = await ctx.connection.getAccountInfo(tempAuthPda);
    expect(authAccountInfo).not.toBeNull();

    // Now remove it
    const removeIx = ctx.client.removeAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      targetAuthority: tempAuthPda,
      refundDestination: ctx.payer.publicKey,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authorizerSigner: ownerKeypair.publicKey,
    });

    const sig = await sendTx(ctx, [removeIx], [ownerKeypair]);
    expect(sig).toBeDefined();
    console.log("RemoveAuthority signature:", sig);

    // Verify it's closed
    authAccountInfo = await ctx.connection.getAccountInfo(tempAuthPda);
    expect(authAccountInfo).toBeNull();
  }, 30_000);
});
