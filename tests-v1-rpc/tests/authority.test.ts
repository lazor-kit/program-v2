import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  AuthorityAccount,
  AuthType,
  Role // <--- Add AuthType, Role
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
  // <--- Add highClient

  beforeAll(async () => {
    ctx = await setupTest();
    // <--- Initialize

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

    console.log("Wallet created for authority tests");
  }, 30_000);

  it("Success: Owner adds an Admin (Ed25519)", async () => {
    const newAdmin = Keypair.generate();

    const { ix } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: newAdmin.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Admin,
      walletPda
    });

    await sendTx(ctx, [ix], [ownerKeypair]);

    const [newAdminPda] = findAuthorityPda(walletPda, newAdmin.publicKey.toBytes());
    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAdminPda);
    expect(acc.role).toBe(1); // Admin
  }, 30_000);

  it("Success: Admin adds a Spender", async () => {
    const spender = Keypair.generate();

    const { ix } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair, // Owner still adds here, or admin if we update signer
      newAuthorityPubkey: spender.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });

    await sendTx(ctx, [ix], [ownerKeypair]);

    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());
    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, spenderPda);
    expect(acc.role).toBe(2); // Spender
  }, 30_000);

  it("Success: Owner adds a Secp256r1 Admin", async () => {
    const credentialIdHash = getRandomSeed();
    const p256Pubkey = new Uint8Array(33);
    crypto.getRandomValues(p256Pubkey);
    p256Pubkey[0] = 0x02;

    const { ix } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: p256Pubkey,
      authType: AuthType.Secp256r1,
      role: Role.Admin,
      walletPda,
      credentialHash: credentialIdHash
    });

    await sendTx(ctx, [ix], [ownerKeypair]);

    const [newAdminPda] = findAuthorityPda(walletPda, credentialIdHash);
    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAdminPda);
    expect(acc.authorityType).toBe(1); // Secp256r1
    expect(acc.role).toBe(1); // Admin
  }, 30_000);

  it("Failure: Admin tries to add an Admin", async () => {
    const admin = Keypair.generate();

    // First, add the Admin
    const { ix: ixAdd } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: admin.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Admin,
      walletPda
    });

    await sendTx(ctx, [ixAdd], [ownerKeypair]);

    const anotherAdmin = Keypair.generate();

    // Admin tries to add another Admin -> should fail
    const { ix: ixFail } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: admin,
      newAuthorityPubkey: anotherAdmin.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Admin,
      walletPda
    });

    const result = await tryProcessInstruction(ctx, [ixFail], [admin]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i); // PermissionDenied
  }, 30_000);

  it("Success: Admin removes a Spender", async () => {
    // Add Admin
    const admin = Keypair.generate();
    const { ix: ixAddAdmin } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: admin.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Admin,
      walletPda
    });
    await sendTx(ctx, [ixAddAdmin], [ownerKeypair]);

    // Add Spender
    const spender = Keypair.generate();
    const { ix: ixAddSpender } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: spender.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });
    await sendTx(ctx, [ixAddSpender], [ownerKeypair]);

    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());

    // Admin removes Spender
    const ixRemove = await ctx.highClient.removeAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: admin,
      authorityToRemovePda: spenderPda,
      refundDestination: ctx.payer.publicKey,
      walletPda
    });
    await sendTx(ctx, [ixRemove], [admin]);

    const info = await ctx.connection.getAccountInfo(spenderPda);
    expect(info).toBeNull();
  }, 30_000);

  it("Failure: Spender tries to remove another Spender", async () => {
    const s1 = Keypair.generate();
    const { ix: ixAdd1 } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: s1.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });
    await sendTx(ctx, [ixAdd1], [ownerKeypair]);

    const s2 = Keypair.generate();
    const { ix: ixAdd2 } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: s2.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });
    await sendTx(ctx, [ixAdd2], [ownerKeypair]);

    const [s1Pda] = findAuthorityPda(walletPda, s1.publicKey.toBytes());
    const [s2Pda] = findAuthorityPda(walletPda, s2.publicKey.toBytes());

    const removeIx = await ctx.highClient.removeAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: s1,
      authorityToRemovePda: s2Pda,
      refundDestination: ctx.payer.publicKey,
      walletPda
    });

    const result = await tryProcessInstruction(ctx, [removeIx], [s1]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i);
  }, 30_000);

  it("Success: Secp256r1 Admin removes a Spender", async () => {
    const { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign, generateAuthenticatorData } = await import("./secp256r1Utils");
    const secpAdmin = await generateMockSecp256r1Signer();
    const [secpAdminPda] = findAuthorityPda(walletPda, secpAdmin.credentialIdHash);

    // Add Secp256r1 Admin
    const { ix: ixAddSecp } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: secpAdmin.publicKeyBytes,
      authType: AuthType.Secp256r1,
      role: Role.Admin,
      walletPda,
      credentialHash: secpAdmin.credentialIdHash
    });
    await sendTx(ctx, [ixAddSecp], [ownerKeypair]);

    // Create a disposable Spender
    const victim = Keypair.generate();
    const [victimPda] = findAuthorityPda(walletPda, victim.publicKey.toBytes());

    const { ix: ixAddVictim } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: victim.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });
    await sendTx(ctx, [ixAddVictim], [ownerKeypair]);

    // Secp256r1 Admin removes the victim
    const removeAuthIx = await ctx.highClient.removeAuthority({
      payer: ctx.payer,
      adminType: AuthType.Secp256r1,
      authorityToRemovePda: victimPda,
      refundDestination: ctx.payer.publicKey,
      walletPda,
      adminCredentialHash: secpAdmin.credentialIdHash,
      adminSignature: new Uint8Array(64) // Dummy, overwritten later
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
    const { ix: ixAdd } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: spender.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });
    await sendTx(ctx, [ixAdd], [ownerKeypair]);

    const victim = Keypair.generate();
    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());

    // Spender tries to add -> should fail
    const { ix: ixFail } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: spender,
      newAuthorityPubkey: victim.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });

    const result = await tryProcessInstruction(ctx, [ixFail], [spender]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i);
  }, 30_000);

  it("Failure: Admin cannot remove Owner", async () => {
    const admin = Keypair.generate();
    const { ix: ixAddAdmin } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: admin.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Admin,
      walletPda
    });
    await sendTx(ctx, [ixAddAdmin], [ownerKeypair]);

    // Admin tries to remove Owner
    const removeIx = await ctx.highClient.removeAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: admin,
      authorityToRemovePda: ownerAuthPda,
      refundDestination: ctx.payer.publicKey,
      walletPda
    });

    const result = await tryProcessInstruction(ctx, [removeIx], [admin]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i);
  }, 30_000);

  it("Failure: Authority from Wallet A cannot add authority to Wallet B", async () => {
    const userSeedB = getRandomSeed();
    const [walletPdaB] = findWalletPda(userSeedB);
    const ownerB = Keypair.generate();

    const { ix: ixCreateB } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: ownerB.publicKey,
      userSeed: userSeedB
    });
    await sendTx(ctx, [ixCreateB]);

    const victim = Keypair.generate();
    const [victimPda] = findAuthorityPda(walletPdaB, victim.publicKey.toBytes());

    const { ix } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair, // Wallet A
      newAuthorityPubkey: victim.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda: walletPdaB // target is B
    });

    const result = await tryProcessInstruction(ctx, [ix], [ownerKeypair]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
  }, 30_000);

  it("Failure: Cannot add same authority twice", async () => {
    const newUser = Keypair.generate();

    const { ix: addIx } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: newUser.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });

    await sendTx(ctx, [addIx], [ownerKeypair]);

    const result = await tryProcessInstruction(ctx, [addIx], [ownerKeypair]);
    expect(result.result).toMatch(/simulation failed|already in use|AccountAlreadyInitialized/i);
  }, 30_000);

  it("Edge: Owner can remove itself (leaves wallet ownerless)", async () => {
    const userSeed2 = getRandomSeed();
    const [wPda] = findWalletPda(userSeed2);
    const [vPda] = findVaultPda(wPda);
    const o = Keypair.generate();
    const [oPda] = findAuthorityPda(wPda, o.publicKey.toBytes());

    const { ix: ixCreate } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o.publicKey,
      userSeed: userSeed2
    });
    await sendTx(ctx, [ixCreate]);

    const removeIx = await ctx.highClient.removeAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: o,
      authorityToRemovePda: oPda,
      refundDestination: ctx.payer.publicKey,
      walletPda: wPda
    });

    await sendTx(ctx, [removeIx], [o]);

    const info = await ctx.connection.getAccountInfo(oPda);
    expect(info).toBeNull();
  }, 30_000);
});
