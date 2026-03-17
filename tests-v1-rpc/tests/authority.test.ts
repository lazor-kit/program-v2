/**
 * LazorKit V1 Client — Authority tests
 *
 * Tests: AddAuthority, RemoveAuthority with Ed25519 and Secp256r1.
 */

import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  AuthorityAccount,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, tryProcessInstruction, tryProcessInstructions, type TestContext, PROGRAM_ID } from "./common";

function getRandomSeed() {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  return seed;
}

describe("LazorKit V1 — Authority", () => {
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

    const [wPda] = findWalletPda(userSeed);
    walletPda = wPda;
    const [vPda] = findVaultPda(walletPda);
    vaultPda = vPda;
    
    let bump;
    const [oPda, oBump] = findAuthorityPda(walletPda, ownerKeypair.publicKey.toBytes());
    ownerAuthPda = oPda;
    bump = oBump;

    const createWalletIx = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: ownerAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authBump: bump,
      authPubkey: ownerKeypair.publicKey.toBytes(),
    });

    await sendTx(ctx, [createWalletIx]);
    console.log("Wallet created for authority tests");
  }, 30_000);

  it("Success: Owner adds an Admin (Ed25519)", async () => {
    const newAdmin = Keypair.generate();
    const [newAdminPda] = findAuthorityPda(walletPda, newAdmin.publicKey.toBytes());

    const ix = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: newAdminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, // Ed25519
      newRole: 1,  // Admin
      authPubkey: newAdmin.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [ix], [ownerKeypair]);
    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAdminPda);
    expect(acc.role).toBe(1); // Admin
  }, 30_000);

  it("Success: Admin adds a Spender", async () => {
    const spender = Keypair.generate();
    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());

    const ix = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: spenderPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      newRole: 2, // Spender
      authPubkey: spender.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [ix], [ownerKeypair]);
    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, spenderPda);
    expect(acc.role).toBe(2); // Spender
  }, 30_000);

  it("Success: Owner adds a Secp256r1 Admin", async () => {
    const credentialIdHash = getRandomSeed();
    const p256Pubkey = new Uint8Array(33);
    crypto.getRandomValues(p256Pubkey);
    p256Pubkey[0] = 0x02;
    const [newAdminPda] = findAuthorityPda(walletPda, credentialIdHash);

    const ix = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: newAdminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 1, // Secp256r1
      newRole: 1,  // Admin
      authPubkey: p256Pubkey,
      credentialHash: credentialIdHash,
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [ix], [ownerKeypair]);
    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAdminPda);
    expect(acc.authorityType).toBe(1); // Secp256r1
    expect(acc.role).toBe(1); // Admin
  }, 30_000);

  it("Failure: Admin tries to add an Admin", async () => {
    const admin = Keypair.generate();
    const [adminPda] = findAuthorityPda(walletPda, admin.publicKey.toBytes());

    // First, add the Admin
    const addAdminIx = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: adminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      newRole: 1,
      authPubkey: admin.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    });
    await sendTx(ctx, [addAdminIx], [ownerKeypair]);

    const anotherAdmin = Keypair.generate();
    const [anotherAdminPda] = findAuthorityPda(walletPda, anotherAdmin.publicKey.toBytes());

    // Admin tries to add another Admin -> should fail
    const ix = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: adminPda,
      newAuthority: anotherAdminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      newRole: 1, // Admin (forbidden for Admin to add Admin)
      authPubkey: anotherAdmin.publicKey.toBytes(),
      authorizerSigner: admin.publicKey,
    });

    const result = await tryProcessInstruction(ctx, ix, [admin]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i); // PermissionDenied
  }, 30_000);

  it("Success: Admin removes a Spender", async () => {
    // Add Admin
    const admin = Keypair.generate();
    const [adminPda] = findAuthorityPda(walletPda, admin.publicKey.toBytes());
    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: adminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      newRole: 1,
      authPubkey: admin.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    // Add Spender
    const spender = Keypair.generate();
    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());
    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: spenderPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      newRole: 2,
      authPubkey: spender.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    // Admin removes Spender
    const removeIx = ctx.client.removeAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: adminPda,
      targetAuthority: spenderPda,
      refundDestination: ctx.payer.publicKey,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authorizerSigner: admin.publicKey,
    });

    await sendTx(ctx, [removeIx], [admin]);
    const info = await ctx.connection.getAccountInfo(spenderPda);
    expect(info).toBeNull();
  }, 30_000);

  it("Failure: Spender tries to remove another Spender", async () => {
    const s1 = Keypair.generate();
    const [s1Pda] = findAuthorityPda(walletPda, s1.publicKey.toBytes());
    const s2 = Keypair.generate();
    const [s2Pda] = findAuthorityPda(walletPda, s2.publicKey.toBytes());

    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: s1Pda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2,
      authPubkey: s1.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: s2Pda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2,
      authPubkey: s2.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    const removeIx = ctx.client.removeAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: s1Pda,
      targetAuthority: s2Pda,
      refundDestination: ctx.payer.publicKey,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authorizerSigner: s1.publicKey,
    });

    const result = await tryProcessInstruction(ctx, removeIx, [s1]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i);
  }, 30_000);

  it("Success: Secp256r1 Admin removes a Spender", async () => {
    const { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign, generateAuthenticatorData } = await import("./secp256r1Utils");
    const secpAdmin = await generateMockSecp256r1Signer();
    const [secpAdminPda] = findAuthorityPda(walletPda, secpAdmin.credentialIdHash);

    // Add Secp256r1 Admin
    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: secpAdminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 1, // Secp256r1
      newRole: 1,
      authPubkey: secpAdmin.publicKeyBytes,
      credentialHash: secpAdmin.credentialIdHash,
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    // Create a disposable Spender
    const victim = Keypair.generate();
    const [victimPda] = findAuthorityPda(walletPda, victim.publicKey.toBytes());
    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: victimPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2,
      authPubkey: victim.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    // Secp256r1 Admin removes the victim
    const removeAuthIx = ctx.client.removeAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: secpAdminPda,
      targetAuthority: victimPda,
      refundDestination: ctx.payer.publicKey,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
    });

    removeAuthIx.keys = [
      ...(removeAuthIx.keys || []),
      { pubkey: new PublicKey("Sysvar1nstructions1111111111111111111111111"), isSigner: false, isWritable: false },
      { pubkey: new PublicKey("SysvarS1otHashes111111111111111111111111111"), isSigner: false, isWritable: false },
    ];

    const slotHashesAddress = new PublicKey("SysvarS1otHashes111111111111111111111111111");
    const accountInfo = await ctx.connection.getAccountInfo(slotHashesAddress);
    const rawData = accountInfo!.data;
    const currentSlot = new DataView(rawData.buffer, rawData.byteOffset, rawData.byteLength).getBigUint64(8, true);

    const sysvarIxIndex = removeAuthIx.keys.length - 2;
    const sysvarSlotIndex = removeAuthIx.keys.length - 1;

    const authenticatorDataRaw = generateAuthenticatorData("example.com");
    const authPayload = buildSecp256r1AuthPayload(sysvarIxIndex, sysvarSlotIndex, authenticatorDataRaw, currentSlot);

    // signedPayload: target_auth_pda (32) + refund_dest (32)
    const signedPayload = new Uint8Array(64);
    signedPayload.set(victimPda.toBytes(), 0);
    signedPayload.set(ctx.payer.publicKey.toBytes(), 32);

    const currentSlotBytes = new Uint8Array(8);
    new DataView(currentSlotBytes.buffer).setBigUint64(0, currentSlot, true);

    const msgToSign = getSecp256r1MessageToSign(
      new Uint8Array([2]), // RemoveAuthority discriminator
      authPayload,
      signedPayload,
      ctx.payer.publicKey.toBytes(),
      PROGRAM_ID.toBytes(),
      authenticatorDataRaw,
      currentSlotBytes
    );

    const sysvarIx = await createSecp256r1Instruction(secpAdmin, msgToSign);

    // Append authPayload to data
    const newIxData = Buffer.alloc(removeAuthIx.data.length + authPayload.length);
    removeAuthIx.data.copy(newIxData, 0);
    newIxData.set(authPayload, removeAuthIx.data.length);
    removeAuthIx.data = newIxData;

    const result = await tryProcessInstructions(ctx, [sysvarIx, removeAuthIx]);
    expect(result.result).toBe("ok");

    const info = await ctx.connection.getAccountInfo(victimPda);
    expect(info).toBeNull();
  }, 30_000);

  it("Failure: Spender cannot add any authority", async () => {
    const spender = Keypair.generate();
    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());
    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: spenderPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2,
      authPubkey: spender.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    const victim = Keypair.generate();
    const [victimPda] = findAuthorityPda(walletPda, victim.publicKey.toBytes());

    const addIx = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: spenderPda,
      newAuthority: victimPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2,
      authPubkey: victim.publicKey.toBytes(),
      authorizerSigner: spender.publicKey,
    });

    const result = await tryProcessInstruction(ctx, addIx, [spender]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i);
  }, 30_000);

  it("Failure: Admin cannot remove Owner", async () => {
    const admin = Keypair.generate();
    const [adminPda] = findAuthorityPda(walletPda, admin.publicKey.toBytes());
    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: adminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 1,
      authPubkey: admin.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    const removeIx = ctx.client.removeAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: adminPda,
      targetAuthority: ownerAuthPda,
      refundDestination: ctx.payer.publicKey,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authorizerSigner: admin.publicKey,
    });

    const result = await tryProcessInstruction(ctx, removeIx, [admin]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i);
  }, 30_000);

  it("Failure: Authority from Wallet A cannot add authority to Wallet B", async () => {
    const userSeedB = getRandomSeed();
    const [walletPdaB] = findWalletPda(userSeedB);
    const [vaultPdaB] = findVaultPda(walletPdaB);
    const ownerB = Keypair.generate();
    const [ownerBAuthPda] = findAuthorityPda(walletPdaB, ownerB.publicKey.toBytes());

    await sendTx(ctx, [ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPdaB, vault: vaultPdaB, authority: ownerBAuthPda,
      config: ctx.configPda, treasuryShard: ctx.treasuryShard,
      userSeed: userSeedB, authType: 0,
      authPubkey: ownerB.publicKey.toBytes(),
    })]);

    const victim = Keypair.generate();
    const [victimPda] = findAuthorityPda(walletPdaB, victim.publicKey.toBytes());

    const ix = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPdaB, // Target is B
      adminAuthority: ownerAuthPda, // Wallet A
      newAuthority: victimPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2,
      authPubkey: victim.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    });

    const result = await tryProcessInstruction(ctx, ix, [ownerKeypair]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
  }, 30_000);

  it("Failure: Cannot add same authority twice", async () => {
    const newUser = Keypair.generate();
    const [newUserPda] = findAuthorityPda(walletPda, newUser.publicKey.toBytes());

    const addIx = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: newUserPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2,
      authPubkey: newUser.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [addIx], [ownerKeypair]);

    const result = await tryProcessInstruction(ctx, addIx, [ownerKeypair]);
    expect(result.result).toMatch(/simulation failed|already in use|AccountAlreadyInitialized/i);
  }, 30_000);

  it("Edge: Owner can remove itself (leaves wallet ownerless)", async () => {
    const userSeed2 = getRandomSeed();
    const [wPda] = findWalletPda(userSeed2);
    const [vPda] = findVaultPda(wPda);
    const o = Keypair.generate();
    const [oPda] = findAuthorityPda(wPda, o.publicKey.toBytes());

    await sendTx(ctx, [ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: wPda, vault: vPda, authority: oPda,
      config: ctx.configPda, treasuryShard: ctx.treasuryShard,
      userSeed: userSeed2, authType: 0,
      authPubkey: o.publicKey.toBytes(),
    })]);

    const removeIx = ctx.client.removeAuthority({
      payer: ctx.payer.publicKey,
      wallet: wPda,
      adminAuthority: oPda,
      targetAuthority: oPda,
      refundDestination: ctx.payer.publicKey,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authorizerSigner: o.publicKey,
    });

    await sendTx(ctx, [removeIx], [o]);
    const info = await ctx.connection.getAccountInfo(oPda);
    expect(info).toBeNull();
  }, 30_000);
});
