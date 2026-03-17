/**
 * LazorKit V1 Client — Wallet tests
 *
 * Tests: CreateWallet, verify account data.
 */

import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, type TestContext } from "./common";

describe("LazorKit V1 — Wallet", () => {
  let ctx: TestContext;

  beforeAll(async () => {
    ctx = await setupTest();
  }, 30_000);

  it("should create a wallet with Ed25519 authority", async () => {
    // 1. Prepare
    const userSeed = new Uint8Array(32);
    crypto.getRandomValues(userSeed);

    // For Ed25519, the authority PDA seed is the authorizer's public key bytes.
    // CreateWallet does NOT require the authorizer to sign — only the payer signs.
    const authorizerKeypair = Keypair.generate();
    const authPubkeyBytes = authorizerKeypair.publicKey.toBytes();

    // 2. Derive PDAs
    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);
    const [adminAuthPda] = findAuthorityPda(walletPda, authPubkeyBytes);

    // 3. Create instruction using LazorWeb3Client
    const ix = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: adminAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0, // Ed25519
      authPubkey: authPubkeyBytes,
    });

    // 4. Send transaction — only payer signs
    const sig = await sendTx(ctx, [ix]);
    expect(sig).toBeDefined();
    console.log("CreateWallet signature:", sig);

    // 5. Verify wallet account was created
    const walletAccountInfo = await ctx.connection.getAccountInfo(walletPda);
    expect(walletAccountInfo).not.toBeNull();
    expect(walletAccountInfo!.owner.toBase58()).toBe(
      "FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao"
    );

    // 6. Verify authority account was created
    const authAccountInfo = await ctx.connection.getAccountInfo(adminAuthPda);
    expect(authAccountInfo).not.toBeNull();

    // 7. Verify vault account was created
    const vaultAccountInfo = await ctx.connection.getAccountInfo(vaultPda);
    expect(vaultAccountInfo).not.toBeNull();
  }, 30_000);

  it("should create a second wallet with different seed", async () => {
    const userSeed = new Uint8Array(32);
    crypto.getRandomValues(userSeed);
    const authorizerKeypair = Keypair.generate();

    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);
    const [adminAuthPda] = findAuthorityPda(
      walletPda,
      authorizerKeypair.publicKey.toBytes()
    );

    const ix = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: adminAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: authorizerKeypair.publicKey.toBytes(),
    });

    const sig = await sendTx(ctx, [ix]);
    expect(sig).toBeDefined();

    const walletAccountInfo = await ctx.connection.getAccountInfo(walletPda);
    expect(walletAccountInfo).not.toBeNull();
  }, 30_000);
});
