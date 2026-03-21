/**
 * 06-ownership.test.ts
 *
 * Tests for: TransferOwnership (Ed25519 + Secp256r1), CloseWallet
 * Merged from: wallet.test.ts (transfer ownership tests) + cleanup.test.ts (close wallet tests)
 */

import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  AuthorityAccount,
  AuthType,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, getRandomSeed, tryProcessInstruction, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";
import { generateMockSecp256r1Signer, buildSecp256r1PrecompileIx, buildAuthPayload, buildSecp256r1Message, buildAuthenticatorData, readCurrentSlot, appendSecp256r1Sysvars } from "./secp256r1Utils";

describe("Ownership & Wallet Lifecycle", () => {
  let ctx: TestContext;
  let ownerKeypair: Keypair;
  let walletPda: PublicKey;
  let ownerAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();
    ownerKeypair = Keypair.generate();

    const { ix, walletPda: w, authorityPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: ownerKeypair.publicKey,
    });
    await sendTx(ctx, [ix]);
    walletPda = w;
    ownerAuthPda = authorityPda;
  }, 30_000);

  // ─── Transfer Ownership ────────────────────────────────────────────────────

  it("Transfer ownership (Ed25519 → Ed25519)", async () => {
    const userSeed = getRandomSeed();
    const o = Keypair.generate();
    const { ix, walletPda: wPda, authorityPda: oPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const newOwner = Keypair.generate();
    const [newAuthPda] = findAuthorityPda(wPda, newOwner.publicKey.toBytes());

    const transferIx = await ctx.highClient.transferOwnership({
      payer: ctx.payer,
      walletPda: wPda,
      currentOwnerAuthority: oPda,
      newOwnerAuthority: newAuthPda,
      authType: AuthType.Ed25519,
      authPubkey: newOwner.publicKey.toBytes(),
      signer: o
    });
    await sendTx(ctx, [transferIx], [o]);

    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAuthPda);
    expect(acc.role).toBe(0);
  }, 30_000);

  it("Failure: Admin cannot transfer ownership", async () => {
    const userSeed = getRandomSeed();
    const o = Keypair.generate();
    const admin = Keypair.generate();

    const { ix, walletPda: wPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const { ix: ixAdd } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: o,
      newAuthorityPubkey: admin.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: 1,
      walletPda: wPda
    });
    await sendTx(ctx, [ixAdd], [o]);

    const [adminPda] = findAuthorityPda(wPda, admin.publicKey.toBytes());
    const newOwner = Keypair.generate();
    const [newAuthPda] = findAuthorityPda(wPda, newOwner.publicKey.toBytes());

    const transferIx = await ctx.highClient.transferOwnership({
      payer: ctx.payer,
      walletPda: wPda,
      currentOwnerAuthority: adminPda,
      newOwnerAuthority: newAuthPda,
      authType: AuthType.Ed25519,
      authPubkey: newOwner.publicKey.toBytes(),
      signer: admin
    });

    const result = await tryProcessInstruction(ctx, [transferIx], [admin]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba|PermissionDenied/i);
  }, 30_000);

  it("Failure: Cannot transfer ownership to zero address", async () => {
    const userSeed = getRandomSeed();
    const o = Keypair.generate();
    const { ix, walletPda: wPda, authorityPda: oPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const zeroKey = new Uint8Array(32);
    const [zeroPda] = findAuthorityPda(wPda, zeroKey);

    const transferIx = await ctx.highClient.transferOwnership({
      payer: ctx.payer,
      walletPda: wPda,
      currentOwnerAuthority: oPda,
      newOwnerAuthority: zeroPda,
      authType: AuthType.Ed25519,
      authPubkey: zeroKey,
      signer: o
    });

    const result = await tryProcessInstruction(ctx, [transferIx], [o]);
    expect(result.result).toMatch(/simulation failed|InvalidArgument|zero|0x0/i);
  }, 30_000);

  it("After transfer ownership, old owner account is closed", async () => {
    const userSeed = getRandomSeed();
    const o = Keypair.generate();
    const { ix, walletPda: wPda, authorityPda: oPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const newOwner = Keypair.generate();
    const [newAuthPda] = findAuthorityPda(wPda, newOwner.publicKey.toBytes());

    const transferIx = await ctx.highClient.transferOwnership({
      payer: ctx.payer,
      walletPda: wPda,
      currentOwnerAuthority: oPda,
      newOwnerAuthority: newAuthPda,
      authType: AuthType.Ed25519,
      authPubkey: newOwner.publicKey.toBytes(),
      signer: o
    });
    await sendTx(ctx, [transferIx], [o]);

    const oldAuthInfo = await ctx.connection.getAccountInfo(oPda);
    expect(oldAuthInfo).toBeNull();
  }, 30_000);

  it("Secp256r1 Owner transfers ownership to Ed25519", async () => {
    const userSeed = getRandomSeed();
    const [wPda] = findWalletPda(userSeed);

    const secpOwner = await generateMockSecp256r1Signer();
    const [secpOwnerPda] = findAuthorityPda(wPda, secpOwner.credentialIdHash);

    const { ix: createIx } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Secp256r1,
      pubkey: secpOwner.publicKeyBytes,
      credentialHash: secpOwner.credentialIdHash,
      userSeed
    });
    await sendTx(ctx, [createIx]);

    const newOwner = Keypair.generate();
    const newOwnerBytes = newOwner.publicKey.toBytes();
    const [newAuthPda] = findAuthorityPda(wPda, newOwnerBytes);

    const transferIx = await ctx.highClient.transferOwnership({
      payer: ctx.payer,
      walletPda: wPda,
      currentOwnerAuthority: secpOwnerPda,
      newOwnerAuthority: newAuthPda,
      authType: AuthType.Ed25519,
      authPubkey: newOwnerBytes,
    });

    const { ix: ixWithSysvars, sysvarIxIndex, sysvarSlotIndex } = appendSecp256r1Sysvars(transferIx);
    ixWithSysvars.keys.push(
      { pubkey: new PublicKey("SysvarRent111111111111111111111111111111111"), isSigner: false, isWritable: false },
    );

    const currentSlot = await readCurrentSlot(ctx.connection);
    const authenticatorData = await buildAuthenticatorData("example.com");
    const authPayload = buildAuthPayload({ sysvarIxIndex, sysvarSlotIndex, authenticatorData, slot: currentSlot });

    const signedPayload = new Uint8Array(1 + 32 + 32);
    signedPayload[0] = 0;
    signedPayload.set(newOwnerBytes, 1);
    signedPayload.set(ctx.payer.publicKey.toBytes(), 33);

    const msgToSign = await buildSecp256r1Message({
      discriminator: 3,
      authPayload, signedPayload,
      payer: ctx.payer.publicKey,
      programId: new PublicKey(PROGRAM_ID),
      slot: currentSlot,
    });

    const sysvarIx = await buildSecp256r1PrecompileIx(secpOwner, msgToSign);

    const originalData = Buffer.from(ixWithSysvars.data);
    ixWithSysvars.data = Buffer.concat([originalData, Buffer.from(authPayload)]);

    const result = await tryProcessInstructions(ctx, [sysvarIx, ixWithSysvars]);
    expect(result.result).toBe("ok");

    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAuthPda);
    expect(acc.role).toBe(0);
  }, 30_000);

  // ─── Close Wallet ──────────────────────────────────────────────────────────

  it("CloseWallet: owner closes wallet and sweeps rent", async () => {
    const owner = Keypair.generate();
    const userSeed = getRandomSeed();

    const { ix, walletPda: wPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const [vPda] = findVaultPda(wPda);
    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vPda, 25_000_000n)]);

    const destWallet = Keypair.generate();
    const closeIx = await ctx.highClient.closeWallet({
      payer: ctx.payer,
      walletPda: wPda,
      destination: destWallet.publicKey,
      adminType: AuthType.Ed25519,
      adminSigner: owner
    });
    await sendTx(ctx, [closeIx], [owner]);

    const destBalance = await ctx.connection.getBalance(destWallet.publicKey);
    expect(destBalance).toBeGreaterThan(25_000_000);
  });

  it("CloseWallet: rejects non-owner closing wallet", async () => {
    const owner = Keypair.generate();
    const attacker = Keypair.generate();
    const { ix: ixCreate, walletPda: wPda, authorityPda: oAuthPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey
    });
    await sendTx(ctx, [ixCreate]);

    const closeWalletIx = await ctx.highClient.closeWallet({
      payer: attacker,
      walletPda: wPda,
      destination: Keypair.generate().publicKey,
      adminType: AuthType.Ed25519,
      adminSigner: attacker,
      adminAuthorityPda: oAuthPda
    });

    const result = await tryProcessInstructions(ctx, [closeWalletIx], [attacker]);
    expect(result.result).not.toBe("ok");
  });

  it("CloseWallet: rejects if destination is the vault PDA", async () => {
    const owner = Keypair.generate();
    const { ix: ixCreate, walletPda: wPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
    });
    await sendTx(ctx, [ixCreate]);

    const [vaultPda_] = findVaultPda(wPda);

    const closeWalletIx = await ctx.highClient.closeWallet({
      payer: ctx.payer,
      walletPda: wPda,
      destination: vaultPda_,
      adminType: AuthType.Ed25519,
      adminSigner: owner
    });

    const result = await tryProcessInstructions(ctx, [closeWalletIx], [ctx.payer, owner]);
    expect(result.result).not.toBe("ok");
  });
});
