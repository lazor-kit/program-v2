import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  AuthorityAccount,
  LazorClient,
  AuthType,
  Role // <--- Add Role
} from "@lazorkit/solita-client";
import { setupTest, sendTx, tryProcessInstruction, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";

function getRandomSeed() {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  return seed;
}

describe("LazorKit V1 — Session", () => {
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
 
    // Fund vault
    const [vPda] = findVaultPda(walletPda);
    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vPda, 500_000_000n)]);
    console.log("Wallet created and funded for session tests");
  }, 30_000);

  it("Success: Owner creates a session key", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const expiresAt = 999999999n;

    const { ix } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey.publicKey,
      expiresAt: expiresAt,
      walletPda
    });

    await sendTx(ctx, [ix], [ownerKeypair]);

    const sessionAcc = await ctx.connection.getAccountInfo(sessionPda);
    expect(sessionAcc).not.toBeNull();
  }, 30_000);

  it("Success: Execution using session key", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const expiresAt = BigInt(2 ** 62); // far future

    const { ix: createIx } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey.publicKey,
      expiresAt: expiresAt,
      walletPda
    });
    await sendTx(ctx, [createIx], [ownerKeypair]);

    const recipient = Keypair.generate().publicKey;
    
    // Execute using session key
    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: sessionPda,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 1_000_000n)
      ],
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
      newAuthorityPubkey: spender.publicKey.toBytes(),
      authType: AuthType.Ed25519,
      role: Role.Spender,
      walletPda
    });
    await sendTx(ctx, [ixAdd], [ownerKeypair]);

    const sessionKey = Keypair.generate();

    const { ix: ixFail } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: spender, // Spender tries to sign
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

    const configPda = PublicKey.findProgramAddressSync([Buffer.from("config")], ctx.highClient.programId)[0];
    const shardId = ctx.payer.publicKey.toBytes().reduce((a: number, b: number) => a + b, 0) % 16;
    const [treasuryShard] = PublicKey.findProgramAddressSync([Buffer.from("treasury"), new Uint8Array([shardId])], ctx.highClient.programId);

    // Explicitly pass sessionPda1 as adminAuthority to test contract account validation
    const ix = ctx.highClient.client.createSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: sessionPda1, // Session PDA
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
    const [newUserPda] = findAuthorityPda(walletPda, newUser.publicKey.toBytes());

    // Explicitly pass sessionPda as adminAuthority to test contract account validation
    const { ix } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      walletPda: walletPda,
      newAuthorityPubkey: newUser.publicKey.toBytes(),
      authType: AuthType.Ed25519, 
      role: Role.Spender,
      adminType: AuthType.Ed25519,
      adminSigner: sessionKey as any,
      adminAuthorityPda: sessionPda
    });

    const result = await tryProcessInstruction(ctx, ix, [sessionKey]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData|0x1770/i);
  }, 30_000);

  it("Success: Secp256r1 Admin creates a session", async () => {
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

    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const expiresAt = 999999999n;

    const { ix: createSessionIx } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Secp256r1,
      adminCredentialHash: secpAdmin.credentialIdHash,
      adminSignature: new Uint8Array(64), // Dummy, overwritten later
      sessionKey: sessionKey.publicKey,
      expiresAt,
      walletPda
    });

    const adminMeta = createSessionIx.keys.find(k => k.pubkey.equals(secpAdminPda));
    if (adminMeta) adminMeta.isWritable = true;

    createSessionIx.keys = [
      ...(createSessionIx.keys || []),
      { pubkey: new PublicKey("Sysvar1nstructions1111111111111111111111111"), isSigner: false, isWritable: false },
      { pubkey: new PublicKey("SysvarS1otHashes111111111111111111111111111"), isSigner: false, isWritable: false },
    ];

    const slotHashesAddress = new PublicKey("SysvarS1otHashes111111111111111111111111111");
    const accountInfo = await ctx.connection.getAccountInfo(slotHashesAddress);
    const rawData = accountInfo!.data;
    const currentSlot = new DataView(rawData.buffer, rawData.byteOffset, rawData.byteLength).getBigUint64(8, true);

    const sysvarIxIndex = createSessionIx.keys.length - 2;
    const sysvarSlotIndex = createSessionIx.keys.length - 1;

    const authenticatorDataRaw = generateAuthenticatorData("example.com");
    const authPayload = buildSecp256r1AuthPayload(sysvarIxIndex, sysvarSlotIndex, authenticatorDataRaw, currentSlot);

    // signedPayload: session_key (32) + expiresAt (8) + payer(32)
    const signedPayload = new Uint8Array(32 + 8 + 32);
    signedPayload.set(sessionKey.publicKey.toBytes(), 0);
    new DataView(signedPayload.buffer).setBigUint64(32, expiresAt, true);
    signedPayload.set(ctx.payer.publicKey.toBytes(), 40);

    const currentSlotBytes = new Uint8Array(8);
    new DataView(currentSlotBytes.buffer).setBigUint64(0, currentSlot, true);

    const msgToSign = getSecp256r1MessageToSign(
      new Uint8Array([5]), // CreateSession
      authPayload,
      signedPayload,
      ctx.payer.publicKey.toBytes(),
      PROGRAM_ID.toBytes(),
      authenticatorDataRaw,
      currentSlotBytes
    );

    const sysvarIx = await createSecp256r1Instruction(secpAdmin, msgToSign);

    // Append authPayload
    const newIxData = Buffer.alloc(createSessionIx.data.length + authPayload.length);
    createSessionIx.data.copy(newIxData, 0);
    newIxData.set(authPayload, createSessionIx.data.length);
    createSessionIx.data = newIxData;

    const result = await tryProcessInstructions(ctx, [sysvarIx, createSessionIx]);
    expect(result.result).toBe("ok");

    const sessionAcc = await ctx.connection.getAccountInfo(sessionPda);
    expect(sessionAcc).not.toBeNull();
  }, 30_000);
});
