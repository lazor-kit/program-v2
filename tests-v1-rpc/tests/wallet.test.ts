import { Keypair, PublicKey, SystemProgram, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  AuthorityAccount,
  LazorClient,
  AuthType // <--- Add AuthType
} from "@lazorkit/solita-client";
import { setupTest, sendTx, tryProcessInstruction, tryProcessInstructions, type TestContext, PROGRAM_ID } from "./common";
import { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign, generateAuthenticatorData } from "./secp256r1Utils";

describe("LazorKit V1 — Wallet Lifecycle", () => {
  let ctx: TestContext;
  let highClient: LazorClient; // <--- Add highClient

  beforeAll(async () => {
    ctx = await setupTest();
    highClient = new LazorClient(ctx.connection); // <--- Initialize
  }, 30_000);

  function getRandomSeed() {
    const seed = new Uint8Array(32);
    crypto.getRandomValues(seed);
    return seed;
  }

  // --- Create Wallet ---

  it("Success: Create wallet with Ed25519 owner", async () => {
    const userSeed = getRandomSeed();
    const owner = Keypair.generate();

    const { ix, walletPda } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed
    });

    await sendTx(ctx, [ix]);

    const [authPda] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

    const authAcc = await AuthorityAccount.fromAccountAddress(ctx.connection, authPda);
    expect(authAcc.authorityType).toBe(0); // Ed25519
    expect(authAcc.role).toBe(0); // Owner
  }, 30_000);

  it("Success: Create wallet with Secp256r1 (WebAuthn) owner", async () => {
    const userSeed = getRandomSeed();
    const credentialIdHash = getRandomSeed();
    const p256Pubkey = new Uint8Array(33).map(() => Math.floor(Math.random() * 256));
    p256Pubkey[0] = 0x02;

    const { ix, walletPda } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Secp256r1,
      pubkey: p256Pubkey,
      credentialHash: credentialIdHash,
      userSeed
    });

    await sendTx(ctx, [ix]);

    const [authPda] = findAuthorityPda(walletPda, credentialIdHash);

    const authAcc = await AuthorityAccount.fromAccountAddress(ctx.connection, authPda);
    expect(authAcc.authorityType).toBe(1); // Secp256r1
    expect(authAcc.role).toBe(0); // Owner
  }, 30_000);

  // --- Discovery ---

  it("Discovery: Ed25519 — pubkey → PDA → wallet", async () => {
    const userSeed = getRandomSeed();
    const owner = Keypair.generate();

    const { ix, walletPda } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed
    });

    await sendTx(ctx, [ix]);

    // Discover
    const discoveredWallets = await LazorClient.findWalletByOwner(ctx.connection, owner.publicKey);
    expect(discoveredWallets).toContainEqual(walletPda);
  }, 30_000);

  // --- Transfer Ownership ---

  it("Success: Transfer ownership (Ed25519 -> Ed25519)", async () => {
    const userSeed = getRandomSeed();
    const currentOwner = Keypair.generate();

    const { ix, walletPda } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: currentOwner.publicKey,
      userSeed
    });

    await sendTx(ctx, [ix]);

    const newOwner = Keypair.generate();
    const newOwnerBytes = newOwner.publicKey.toBytes();
    const [currentAuthPda] = findAuthorityPda(walletPda, currentOwner.publicKey.toBytes());
    const [newAuthPda] = findAuthorityPda(walletPda, newOwnerBytes);

    const ixTransfer = await highClient.transferOwnership({
      payer: ctx.payer,
      walletPda,
      currentOwnerAuthority: currentAuthPda,
      newOwnerAuthority: newAuthPda,
      authType: AuthType.Ed25519,
      authPubkey: newOwnerBytes,
      signer: currentOwner
    });

    await sendTx(ctx, [ixTransfer], [currentOwner]);

    const acc = await AuthorityAccount.fromAccountAddress(ctx.connection, newAuthPda);
    expect(acc.role).toBe(0); // Owner
  }, 30_000);

  it("Failure: Admin cannot transfer ownership", async () => {
    const userSeed = getRandomSeed();
    const owner = Keypair.generate();

    const { ix, walletPda } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed
    });

    await sendTx(ctx, [ix]);

    const [ownerAuthPda] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

    // Add Admin
    const admin = Keypair.generate();
    const [adminPda] = findAuthorityPda(walletPda, admin.publicKey.toBytes());

    const { ix: ixAdd } = await highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      newAuthorityPubkey: admin.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: 1, // Admin (Wait, I should use Role.Admin if I can import it)
      walletPda
    });

    await sendTx(ctx, [ixAdd], [owner]);

    // Admin tries to transfer
    const transferIx = await highClient.transferOwnership({
      payer: ctx.payer,
      walletPda,
      currentOwnerAuthority: adminPda,
      newOwnerAuthority: adminPda, // Irrelevant
      authType: AuthType.Ed25519,
      authPubkey: admin.publicKey.toBytes(),
      signer: admin,
    });

    const result = await tryProcessInstruction(ctx, [transferIx], [admin]);
    expect(result.result).toMatch(/simulation failed|0xbba|3002/i);
  }, 30_000);

  // --- Duplicate Wallet Creation ---

  it("Failure: Cannot create wallet with same seed twice", async () => {
    const userSeed = getRandomSeed();
    const o = Keypair.generate();
    const { ix: createIx } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o.publicKey,
      userSeed
    });

    await sendTx(ctx, [createIx]);

    // Second creation
    const o2 = Keypair.generate();
    const { ix: create2Ix } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o2.publicKey,
      userSeed
    });

    const result = await tryProcessInstruction(ctx, [create2Ix]);
    expect(result.result).toMatch(/simulation failed|already in use|AccountAlreadyInitialized/i);
  }, 30_000);

  // --- Zero-Address Transfer Ownership ---

  it("Failure: Cannot transfer ownership to zero address", async () => {
    const userSeed = getRandomSeed();
    const o = Keypair.generate();

    const { ix: createIx, walletPda } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o.publicKey,
      userSeed
    });

    await sendTx(ctx, [createIx]);

    const zeroPubkey = new Uint8Array(32).fill(0);
    const [zeroPda] = findAuthorityPda(walletPda, zeroPubkey);
    const [aPda] = findAuthorityPda(walletPda, o.publicKey.toBytes());

    const transferIx = await highClient.transferOwnership({
      payer: ctx.payer,
      walletPda,
      currentOwnerAuthority: aPda,
      newOwnerAuthority: zeroPda,
      authType: AuthType.Ed25519,
      authPubkey: zeroPubkey,
      signer: o,
    });

    const result = await tryProcessInstruction(ctx, [transferIx], [o]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
  }, 30_000);

  // --- P4: Verification ---

  it("Success: After transfer ownership, old owner account is closed", async () => {
    const userSeed = getRandomSeed();
    const oldOwner = Keypair.generate();

    const { ix: createIx, walletPda } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: oldOwner.publicKey,
      userSeed
    });

    await sendTx(ctx, [createIx]);

    const newOwner = Keypair.generate();
    const newBytes = newOwner.publicKey.toBytes();
    const [oldPda] = findAuthorityPda(walletPda, oldOwner.publicKey.toBytes());
    const [newPda] = findAuthorityPda(walletPda, newBytes);

    const transferIx = await highClient.transferOwnership({
      payer: ctx.payer,
      walletPda,
      currentOwnerAuthority: oldPda,
      newOwnerAuthority: newPda,
      authType: AuthType.Ed25519,
      authPubkey: newBytes,
      signer: oldOwner,
    });

    await sendTx(ctx, [transferIx], [oldOwner]);

    const oldAcc = await ctx.connection.getAccountInfo(oldPda);
    expect(oldAcc).toBeNull();
  }, 30_000);

  it("Success: Secp256r1 Owner transfers ownership to Ed25519", async () => {
    const userSeed = getRandomSeed();
    const [walletPda] = findWalletPda(userSeed);

    // 1. Create Wallet with Secp256r1 Owner
    const secpOwner = await generateMockSecp256r1Signer();
    const [secpOwnerPda] = findAuthorityPda(walletPda, secpOwner.credentialIdHash);

    const { ix: createIx } = await highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Secp256r1,
      pubkey: secpOwner.publicKeyBytes,
      credentialHash: secpOwner.credentialIdHash,
      userSeed
    });

    await sendTx(ctx, [createIx]);

    // 2. Prepare new Ed25519 Owner
    const newOwner = Keypair.generate();
    const newOwnerBytes = newOwner.publicKey.toBytes();
    const [newAuthPda] = findAuthorityPda(walletPda, newOwnerBytes);

    // 3. Perform Transfer
    const transferIx = await highClient.transferOwnership({
      payer: ctx.payer,
      walletPda,
      currentOwnerAuthority: secpOwnerPda,
      newOwnerAuthority: newAuthPda,
      authType: AuthType.Ed25519,
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
