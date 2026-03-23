/**
 * 05-session.test.ts
 *
 * Tests for: CreateSession, CloseSession (Ed25519 + Secp256r1)
 * Merged from: session.test.ts + close-session tests from cleanup.test.ts
 */

import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  SessionAccount,
  AuthType,
  Role,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, getRandomSeed, tryProcessInstruction, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";

describe("Session Management", () => {
  let ctx: TestContext;
  let ownerKeypair: Keypair;
  let userSeed: Uint8Array;
  let walletPda: PublicKey;
  let vaultPda: PublicKey;
  let ownerAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();
    ownerKeypair = Keypair.generate();
    userSeed = getRandomSeed();

    const { ix, walletPda: w } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: ownerKeypair.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);
    walletPda = w;

    const [v] = findVaultPda(walletPda);
    vaultPda = v;

    const [oPda] = findAuthorityPda(walletPda, ownerKeypair.publicKey.toBytes());
    ownerAuthPda = oPda;

    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPda, 500_000_000n)]);
  }, 30_000);

  // ─── Create Session ────────────────────────────────────────────────────────

  it("Success: Owner creates a session key", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const { ix } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey.publicKey,
      expiresAt: 999999999n,
      walletPda
    });

    await sendTx(ctx, [ix], [ownerKeypair]);

    const sessionAccInfo = await ctx.connection.getAccountInfo(sessionPda);
    expect(sessionAccInfo).not.toBeNull();

    // Verification of data mapping using Solita classes
    const sessionAcc = await SessionAccount.fromAccountAddress(ctx.connection, sessionPda);
    expect(sessionAcc.sessionKey.toBase58()).toBe(sessionKey.publicKey.toBase58());
    expect(sessionAcc.wallet.toBase58()).toBe(walletPda.toBase58());
    expect(sessionAcc.expiresAt.toString()).toBe("999999999");
  }, 30_000);

  it("Success: Execution using session key", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const { ix: createIx } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey.publicKey,
      expiresAt: BigInt(2 ** 62),
      walletPda
    });
    await sendTx(ctx, [createIx], [ownerKeypair]);

    const recipient = Keypair.generate().publicKey;
    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: sessionPda,
      innerInstructions: [getSystemTransferIx(vaultPda, recipient, 1_000_000n)],
      signer: sessionKey
    });
    await sendTx(ctx, [executeIx], [sessionKey]);

    const balance = await ctx.connection.getBalance(recipient);
    expect(balance).toBe(1_000_000);
  }, 30_000);

  it("Failure: Spender cannot create session", async () => {
    const spender = Keypair.generate();
    const { ix: ixAdd } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthPubkey: spender.publicKey.toBytes(),
      newAuthType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });
    await sendTx(ctx, [ixAdd], [ownerKeypair]);

    const sessionKey = Keypair.generate();
    const { ix: ixFail } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: spender,
      sessionKey: sessionKey.publicKey,
      expiresAt: BigInt(2 ** 62),
      walletPda
    });

    const result = await tryProcessInstruction(ctx, [ixFail], [spender]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i);
  }, 30_000);

  it("Failure: Session PDA cannot create another session", async () => {
    const sessionKey1 = Keypair.generate();
    const [sessionPda1] = findSessionPda(walletPda, sessionKey1.publicKey);

    const { ix: ixCreate1 } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey1.publicKey,
      expiresAt: BigInt(2 ** 62),
      walletPda
    });
    await sendTx(ctx, [ixCreate1], [ownerKeypair]);

    const sessionKey2 = Keypair.generate();
    const [sessionPda2] = findSessionPda(walletPda, sessionKey2.publicKey);

    const configPda = ctx.highClient.getConfigPda();
    const shardId = ctx.payer.publicKey.toBytes().reduce((a: number, b: number) => a + b, 0) % 16;
    const treasuryShard = ctx.highClient.getTreasuryShardPda(shardId);

    const ix = ctx.highClient.builder.createSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: sessionPda1,
      session: sessionPda2,
      config: configPda,
      treasuryShard: treasuryShard,
      sessionKey: Array.from(sessionKey2.publicKey.toBytes()),
      expiresAt: BigInt(2 ** 62),
      authorizerSigner: sessionKey1.publicKey,
    });

    const result = await tryProcessInstruction(ctx, ix, [sessionKey1]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
  }, 30_000);

  it("Failure: Session key cannot add authority", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const { ix: ixCreate } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey.publicKey,
      expiresAt: BigInt(2 ** 62),
      walletPda
    });
    await sendTx(ctx, [ixCreate], [ownerKeypair]);

    const newUser = Keypair.generate();

    const { ix } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      walletPda: walletPda,
      newAuthPubkey: newUser.publicKey.toBytes(),
      newAuthType: AuthType.Ed25519,
      role: Role.Spender,
      adminType: AuthType.Ed25519,
      adminSigner: sessionKey as any,
      adminAuthorityPda: sessionPda
    });

    const result = await tryProcessInstruction(ctx, ix, [sessionKey]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData|0x1770/i);
  }, 30_000);

  it("Success: Secp256r1 Admin creates a session", async () => {
    const { generateMockSecp256r1Signer, buildAuthPayload, buildSecp256r1Message, buildSecp256r1PrecompileIx, buildAuthenticatorData, readCurrentSlot, appendSecp256r1Sysvars } = await import("./secp256r1Utils");
    const secpAdmin = await generateMockSecp256r1Signer();
    const [secpAdminPda] = findAuthorityPda(walletPda, secpAdmin.credentialIdHash);

    const { ix: ixAddSecp } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthPubkey: secpAdmin.publicKeyBytes,
      newAuthType: AuthType.Secp256r1,
      role: Role.Admin,
      walletPda,
      newCredentialHash: secpAdmin.credentialIdHash
    });
    await sendTx(ctx, [ixAddSecp], [ownerKeypair]);

    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);
    const expiresAt = 999999999n;

    const { ix: createSessionIx } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Secp256r1,
      adminCredentialHash: secpAdmin.credentialIdHash,
      sessionKey: sessionKey.publicKey,
      expiresAt,
      walletPda
    });

    const adminMeta = createSessionIx.keys.find(k => k.pubkey.equals(secpAdminPda));
    if (adminMeta) adminMeta.isWritable = true;

    const { ix: ixWithSysvars, sysvarIxIndex, sysvarSlotIndex } = appendSecp256r1Sysvars(createSessionIx);
    const currentSlot = await readCurrentSlot(ctx.connection);

    const authenticatorData = await buildAuthenticatorData("example.com");
    const authPayload = buildAuthPayload({ sysvarIxIndex, sysvarSlotIndex, authenticatorData, slot: currentSlot });

    const signedPayload = new Uint8Array(32 + 8 + 32);
    signedPayload.set(sessionKey.publicKey.toBytes(), 0);
    new DataView(signedPayload.buffer).setBigUint64(32, expiresAt, true);
    signedPayload.set(ctx.payer.publicKey.toBytes(), 40);

    const msgToSign = await buildSecp256r1Message({
      discriminator: 5,
      authPayload, signedPayload,
      payer: ctx.payer.publicKey,
      programId: PROGRAM_ID,
      slot: currentSlot,
    });

    const sysvarIx = await buildSecp256r1PrecompileIx(secpAdmin, msgToSign);

    const newIxData = Buffer.alloc(ixWithSysvars.data.length + authPayload.length);
    ixWithSysvars.data.copy(newIxData, 0);
    newIxData.set(authPayload, ixWithSysvars.data.length);
    ixWithSysvars.data = newIxData;

    const result = await tryProcessInstructions(ctx, [sysvarIx, ixWithSysvars]);
    expect(result.result).toBe("ok");

    const sessionAcc = await ctx.connection.getAccountInfo(sessionPda);
    expect(sessionAcc).not.toBeNull();
  }, 30_000);

  it("Failure: Execution fails if the session has been closed", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const { ix: createIx } = await ctx.highClient.createSession({
      payer: ctx.payer, adminType: AuthType.Ed25519, adminSigner: ownerKeypair, sessionKey: sessionKey.publicKey, expiresAt: 9999999999n, walletPda
    });
    await sendTx(ctx, [createIx], [ownerKeypair]);

    // Close the session immediately
    const closeIx = await ctx.highClient.closeSession({
      payer: ctx.payer, walletPda, sessionPda, authorizer: { authorizerPda: ownerAuthPda, signer: ownerKeypair }
    });
    await sendTx(ctx, [closeIx], [ownerKeypair]);

    const recipient = Keypair.generate().publicKey;
    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer, walletPda, authorityPda: sessionPda, innerInstructions: [getSystemTransferIx(vaultPda, recipient, 100n)], signer: sessionKey
    });

    const result = await tryProcessInstruction(ctx, [executeIx], [sessionKey]);
    expect(result.result).toMatch(/simulation failed|UninitializedAccount|InvalidAccountData|0xbc4/i);
  }, 30_000);

  // ─── Close Session ─────────────────────────────────────────────────────────

  it("CloseSession: wallet owner closes an active session", async () => {
    const owner = Keypair.generate();
    const { ix: ixCreate, walletPda: wPda, authorityPda: oAuthPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey
    });
    await sendTx(ctx, [ixCreate]);

    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(wPda, sessionKey.publicKey);

    const { ix: ixCreateSession } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      sessionKey: sessionKey.publicKey,
      expiresAt: BigInt(Math.floor(Date.now() / 1000) + 3600),
      walletPda: wPda
    });
    await sendTx(ctx, [ixCreateSession], [owner]);

    const closeSessionIx = await ctx.highClient.closeSession({
      payer: ctx.payer,
      walletPda: wPda,
      sessionPda,
      authorizer: { authorizerPda: oAuthPda, signer: owner }
    });

    const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer, owner]);
    expect(result.result).toBe("ok");
  });

  it("CloseSession: cranker (anyone) closes an expired session", async () => {
    const owner = Keypair.generate();
    const { ix: ixCreate, walletPda: wPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey
    });
    await sendTx(ctx, [ixCreate]);

    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(wPda, sessionKey.publicKey);

    const { ix: ixCreateSession } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      sessionKey: sessionKey.publicKey,
      expiresAt: 0n,
      walletPda: wPda
    });
    await sendTx(ctx, [ixCreateSession], [owner]);

    const closeSessionIx = await ctx.highClient.closeSession({
      payer: ctx.payer,
      walletPda: wPda,
      sessionPda,
    });

    const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer]);
    expect(result.result).toBe("ok");
  });

  it("CloseSession: rejects cranker (anyone) closing an active session", async () => {
    const owner = Keypair.generate();
    const { ix: ixCreate, walletPda: wPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey
    });
    await sendTx(ctx, [ixCreate]);

    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(wPda, sessionKey.publicKey);

    const { ix: ixCreateSession } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      sessionKey: sessionKey.publicKey,
      expiresAt: BigInt(Math.floor(Date.now() / 1000) + 3600),
      walletPda: wPda
    });
    await sendTx(ctx, [ixCreateSession], [owner]);

    const closeSessionIx = await ctx.highClient.closeSession({
      payer: ctx.payer,
      walletPda: wPda,
      sessionPda,
    });

    const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer]);
    expect(result.result).not.toBe("ok");
  });
});
