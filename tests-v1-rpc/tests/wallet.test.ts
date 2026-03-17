import { Keypair, PublicKey, SystemProgram, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  AuthorityAccount,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, tryProcessInstruction, tryProcessInstructions, type TestContext, PROGRAM_ID } from "./common";
import { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign, generateAuthenticatorData } from "./secp256r1Utils";

describe("LazorKit V1 — Wallet Lifecycle", () => {
  let ctx: TestContext;

  beforeAll(async () => {
    ctx = await setupTest();
  }, 30_000);

  function getRandomSeed() {
    const seed = new Uint8Array(32);
    crypto.getRandomValues(seed);
    return seed;
  }

  // --- Create Wallet ---

  it("Success: Create wallet with Ed25519 owner", async () => {
    const userSeed = getRandomSeed();
    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);

    const owner = Keypair.generate();
    const ownerBytes = owner.publicKey.toBytes();
    const [authPda, authBump] = findAuthorityPda(walletPda, ownerBytes);

    const ix = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: authPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0, // Ed25519
      authPubkey: ownerBytes,
    });

    const sig = await sendTx(ctx, [ix]);
    expect(sig).toBeDefined();

    const authAcc = await AuthorityAccount.fromAccountAddress(ctx.connection, authPda);
    expect(authAcc.authorityType).toBe(0); // Ed25519
    expect(authAcc.role).toBe(0); // Owner
  }, 30_000);

  it("Success: Create wallet with Secp256r1 (WebAuthn) owner", async () => {
    const userSeed = getRandomSeed();
    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);

    const credentialIdHash = getRandomSeed();
    const p256Pubkey = new Uint8Array(33).map(() => Math.floor(Math.random() * 256));
    p256Pubkey[0] = 0x02;

    const [authPda, authBump] = findAuthorityPda(walletPda, credentialIdHash);

    const ix = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: authPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 1, // Secp256r1
      authPubkey: p256Pubkey,
      credentialHash: credentialIdHash,
    });

    // In CreateWallet layout, credentialHash is usually passed if it is required on-chain layout,
    // but in V1 client implementation of createWallet, it may only need authPubkey if it matches structure.
    // Let's rely on LazorWeb3Client defaults.
    const sig = await sendTx(ctx, [ix]);
    expect(sig).toBeDefined();

    const authAcc = await AuthorityAccount.fromAccountAddress(ctx.connection, authPda);
    expect(authAcc.authorityType).toBe(1); // Secp256r1
    expect(authAcc.role).toBe(0); // Owner
  }, 30_000);

  // --- Discovery ---

  it("Discovery: Ed25519 — pubkey → PDA → wallet", async () => {
    const userSeed = getRandomSeed();
    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);
    
    const owner = Keypair.generate();
    const ownerBytes = owner.publicKey.toBytes();
    const [authPda] = findAuthorityPda(walletPda, ownerBytes);

    const ix = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: authPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: ownerBytes,
    });

    await sendTx(ctx, [ix]);

    // Discover
    const discoveredAuth = await ctx.client.getAuthorityByPublicKey(ctx.connection, walletPda, owner.publicKey);
    expect(discoveredAuth).not.toBeNull();
    // getAuthorityByPublicKey usually returns account address state representation in V1 Client.
    // Let's verify it is defined.
    expect(discoveredAuth).toBeDefined();
  }, 30_000);

  // --- Transfer Ownership ---

  it("Success: Transfer ownership (Ed25519 -> Ed25519)", async () => {
    const userSeed = getRandomSeed();
    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);
    
    const currentOwner = Keypair.generate();
    const currentOwnerBytes = currentOwner.publicKey.toBytes();
    const [currentAuthPda] = findAuthorityPda(walletPda, currentOwnerBytes);

    const createIx = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: currentAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: currentOwnerBytes,
    });

    await sendTx(ctx, [createIx]);

    const newOwner = Keypair.generate();
    const newOwnerBytes = newOwner.publicKey.toBytes();
    const [newAuthPda] = findAuthorityPda(walletPda, newOwnerBytes);

    const transferIx = ctx.client.transferOwnership({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      currentOwnerAuthority: currentAuthPda,
      newOwnerAuthority: newAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      authPubkey: newOwnerBytes,
      authorizerSigner: currentOwner.publicKey,
    });

    const fs = require("fs");
    const keysLog = transferIx.keys.map((k,i) => `${i}: ${k.pubkey.toBase58()}`).join("\n");
    fs.writeFileSync("/tmp/keys.log", keysLog);
    
    await sendTx(ctx, [transferIx], [currentOwner]);

    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAuthPda);
    expect(acc.role).toBe(0); // Owner
  }, 30_000);

  it("Failure: Admin cannot transfer ownership", async () => {
    const userSeed = getRandomSeed();
    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);
    
    const owner = Keypair.generate();
    const ownerBytes = owner.publicKey.toBytes();
    const [ownerAuthPda] = findAuthorityPda(walletPda, ownerBytes);

    const createIx = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: ownerAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: ownerBytes,
    });

    await sendTx(ctx, [createIx]);

    // Add Admin
    const admin = Keypair.generate();
    const adminBytes = admin.publicKey.toBytes();
    const [adminPda] = findAuthorityPda(walletPda, adminBytes);

    const addAuthIx = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: adminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      newRole: 1, // Admin
      authPubkey: adminBytes,
      authorizerSigner: owner.publicKey,
    });

    await sendTx(ctx, [addAuthIx], [owner]);

    // Admin tries to transfer
    const transferIx = ctx.client.transferOwnership({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      currentOwnerAuthority: adminPda,
      newOwnerAuthority: adminPda, // Irrelevant
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      authPubkey: adminBytes,
      authorizerSigner: admin.publicKey,
    });

    const result = await tryProcessInstruction(ctx, [transferIx], [admin]);
    expect(result.result).toMatch(/simulation failed|0xbba|3002/i);
  }, 30_000);

  // --- Duplicate Wallet Creation ---

  it("Failure: Cannot create wallet with same seed twice", async () => {
    const userSeed = getRandomSeed();
    const [wPda] = findWalletPda(userSeed);
    const [vPda] = findVaultPda(wPda);
    
    const o = Keypair.generate();
    const oBytes = o.publicKey.toBytes();
    const [aPda] = findAuthorityPda(wPda, oBytes);

    const createIx = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: wPda,
      vault: vPda,
      authority: aPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: oBytes,
    });

    await sendTx(ctx, [createIx]);

    // Second creation
    const o2 = Keypair.generate();
    const o2Bytes = o2.publicKey.toBytes();
    const [a2Pda] = findAuthorityPda(wPda, o2Bytes);

    const create2Ix = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: wPda,
      vault: vPda,
      authority: a2Pda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: o2Bytes,
    });

    const result = await tryProcessInstruction(ctx, [create2Ix]);
    expect(result.result).toMatch(/simulation failed|already in use|AccountAlreadyInitialized/i);
  }, 30_000);

  // --- Zero-Address Transfer Ownership ---

  it("Failure: Cannot transfer ownership to zero address", async () => {
    const userSeed = getRandomSeed();
    const [wPda] = findWalletPda(userSeed);
    const [vPda] = findVaultPda(wPda);
    
    const o = Keypair.generate();
    const oBytes = o.publicKey.toBytes();
    const [aPda] = findAuthorityPda(wPda, oBytes);

    const createIx = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: wPda,
      vault: vPda,
      authority: aPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: oBytes,
    });

    await sendTx(ctx, [createIx]);

    const zeroPubkey = new Uint8Array(32).fill(0);
    const [zeroPda] = findAuthorityPda(wPda, zeroPubkey);

    const transferIx = ctx.client.transferOwnership({
      payer: ctx.payer.publicKey,
      wallet: wPda,
      currentOwnerAuthority: aPda,
      newOwnerAuthority: zeroPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      authPubkey: zeroPubkey,
      authorizerSigner: o.publicKey,
    });

    const result = await tryProcessInstruction(ctx, [transferIx], [o]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
  }, 30_000);

  // --- P4: Verification ---

  it("Success: After transfer ownership, old owner account is closed", async () => {
    const userSeed = getRandomSeed();
    const [wPda] = findWalletPda(userSeed);
    const [vPda] = findVaultPda(wPda);
    
    const oldOwner = Keypair.generate();
    const oldBytes = oldOwner.publicKey.toBytes();
    const [oldPda] = findAuthorityPda(wPda, oldBytes);

    const createIx = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: wPda,
      vault: vPda,
      authority: oldPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 0,
      authPubkey: oldBytes,
    });

    await sendTx(ctx, [createIx]);

    const newOwner = Keypair.generate();
    const newBytes = newOwner.publicKey.toBytes();
    const [newPda] = findAuthorityPda(wPda, newBytes);

    const transferIx = ctx.client.transferOwnership({
      payer: ctx.payer.publicKey,
      wallet: wPda,
      currentOwnerAuthority: oldPda,
      newOwnerAuthority: newPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      authPubkey: newBytes,
      authorizerSigner: oldOwner.publicKey,
    });

    await sendTx(ctx, [transferIx], [oldOwner]);

    const oldAcc = await ctx.connection.getAccountInfo(oldPda);
    expect(oldAcc).toBeNull();
  }, 30_000);

  it("Success: Secp256r1 Owner transfers ownership to Ed25519", async () => {
    const userSeed = getRandomSeed();
    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);

    // 1. Create Wallet with Secp256r1 Owner
    const secpOwner = await generateMockSecp256r1Signer();
    const [secpOwnerPda, ownerBump] = findAuthorityPda(walletPda, secpOwner.credentialIdHash);

    const createIx = ctx.client.createWallet({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      vault: vaultPda,
      authority: secpOwnerPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      userSeed,
      authType: 1, // Secp256r1
      authPubkey: secpOwner.publicKeyBytes,
      credentialHash: secpOwner.credentialIdHash,
    });

    await sendTx(ctx, [createIx]);

    // 2. Prepare new Ed25519 Owner
    const newOwner = Keypair.generate();
    const newOwnerBytes = newOwner.publicKey.toBytes();
    const [newAuthPda] = findAuthorityPda(walletPda, newOwnerBytes);

    // 3. Perform Transfer
    const transferIx = ctx.client.transferOwnership({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      currentOwnerAuthority: secpOwnerPda,
      newOwnerAuthority: newAuthPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0,
      authPubkey: newOwnerBytes,
    });

    // Append sysvars
    transferIx.keys = [
      ...(transferIx.keys || []),
      { pubkey: new PublicKey("Sysvar1nstructions1111111111111111111111111"), isSigner: false, isWritable: false },
      { pubkey: new PublicKey("SysvarS1otHashes111111111111111111111111111"), isSigner: false, isWritable: false },
      { pubkey: new PublicKey("SysvarRent111111111111111111111111111111111"), isSigner: false, isWritable: false },
    ];

    const slotHashesAddress = new PublicKey("SysvarS1otHashes111111111111111111111111111");
    const accountInfo = await ctx.connection.getAccountInfo(slotHashesAddress);
    const rawData = Buffer.from(accountInfo!.data);
    const currentSlot = rawData.readBigUInt64LE(8);

    // Indices based on layout (SysvarInstructions is 1st sysvar added, SlotHashes is 2nd, Rent is 3rd)
    // Precompiles iterate account keys. In Solita compact layout they can be populated differently.
    // Let's rely on standard sysvar indices.
    const sysvarIxIndex = transferIx.keys.length - 3; 
    const sysvarSlotIndex = transferIx.keys.length - 2;

    const authenticatorDataRaw = generateAuthenticatorData("example.com");
    const authPayload = buildSecp256r1AuthPayload(sysvarIxIndex, sysvarSlotIndex, authenticatorDataRaw, currentSlot);

    const signedPayload = new Uint8Array(1 + 32 + 32);
    signedPayload[0] = 0; // New type Ed25519
    signedPayload.set(newOwnerBytes, 1);
    signedPayload.set(ctx.payer.publicKey.toBytes(), 33);

    const currentSlotBytes = new Uint8Array(8);
    new DataView(currentSlotBytes.buffer).setBigUint64(0, currentSlot, true);

    const discriminator = new Uint8Array([3]); // TransferOwnership is 3
    const msgToSign = getSecp256r1MessageToSign(
      discriminator,
      authPayload,
      signedPayload,
      ctx.payer.publicKey.toBytes(),
      new PublicKey(PROGRAM_ID).toBytes(),
      authenticatorDataRaw,
      currentSlotBytes
    );

    const sysvarIx = await createSecp256r1Instruction(secpOwner, msgToSign);

    // Pack payload onto transferIx.data
    const originalData = Buffer.from(transferIx.data);
    const finalTransferData = Buffer.concat([originalData, Buffer.from(authPayload)]);
    transferIx.data = finalTransferData;

    const result = await tryProcessInstructions(ctx, [sysvarIx, transferIx]);
    expect(result.result).toBe("ok");

    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAuthPda);
    expect(acc.role).toBe(0); // Owner
  }, 30_000);
});
