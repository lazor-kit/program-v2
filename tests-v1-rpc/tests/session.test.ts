import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  AuthorityAccount,
  LazorClient,
  AuthType // <--- Add AuthType
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
  let highClient: LazorClient; // <--- Add highClient

  beforeAll(async () => {
    ctx = await setupTest();
    highClient = new LazorClient(ctx.connection); // <--- Initialize

    ownerKeypair = Keypair.generate();
    userSeed = getRandomSeed();

    const { ix, walletPda: w } = await highClient.createWallet({
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

    const { ix } = await highClient.createSession({
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

    await highClient.createSession({
      payer: ctx.payer,
      adminType: 0,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey.publicKey,
      expiresAt: expiresAt,
      walletPda
    });

    const recipient = Keypair.generate().publicKey;
    
    // Execute using session key
    await highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: sessionPda,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 1_000_000n)
      ],
      signer: sessionKey
    });

    const balance = await ctx.connection.getBalance(recipient);
    expect(balance).toBe(1_000_000);
  }, 30_000);

  it("Failure: Spender cannot create session", async () => {
    const spender = Keypair.generate();

    await highClient.addAuthority({
      payer: ctx.payer,
      adminType: 0,
      adminSigner: ownerKeypair,
      newAuthorityPubkey: spender.publicKey.toBytes(),
      authType: 0,
      role: 2, // Spender
      walletPda
    });

    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const configPda = PublicKey.findProgramAddressSync([Buffer.from("config")], highClient.programId)[0];
    const shardId = ctx.payer.publicKey.toBytes().reduce((a: number, b: number) => a + b, 0) % 16;
    const [treasuryShard] = PublicKey.findProgramAddressSync([Buffer.from("treasury"), new Uint8Array([shardId])], highClient.programId);

    const ix = highClient.client.createSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: spenderPda, // Spender
      session: sessionPda,
      config: configPda,
      treasuryShard: treasuryShard,
      sessionKey: Array.from(sessionKey.publicKey.toBytes()),
      expiresAt: BigInt(2 ** 62),
      authorizerSigner: spender.publicKey,
    });

    const result = await tryProcessInstruction(ctx, ix, [spender]);
    expect(result.result).toMatch(/simulation failed|3002|0xbba/i);
  }, 30_000);

  it("Failure: Session PDA cannot create another session", async () => {
    const sessionKey1 = Keypair.generate();
    const [sessionPda1] = findSessionPda(walletPda, sessionKey1.publicKey);

    await highClient.createSession({
      payer: ctx.payer,
      adminType: 0,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey1.publicKey,
      expiresAt: BigInt(2 ** 62),
      walletPda
    });

    const sessionKey2 = Keypair.generate();
    const [sessionPda2] = findSessionPda(walletPda, sessionKey2.publicKey);

    const configPda = PublicKey.findProgramAddressSync([Buffer.from("config")], highClient.programId)[0];
    const shardId = ctx.payer.publicKey.toBytes().reduce((a: number, b: number) => a + b, 0) % 16;
    const [treasuryShard] = PublicKey.findProgramAddressSync([Buffer.from("treasury"), new Uint8Array([shardId])], highClient.programId);

    const ix = highClient.client.createSession({
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

    await sendTx(ctx, [ctx.client.createSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      session: sessionPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      sessionKey: Array.from(sessionKey.publicKey.toBytes()),
      expiresAt: BigInt(2 ** 62),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    const newUser = Keypair.generate();
    const [newUserPda] = findAuthorityPda(walletPda, newUser.publicKey.toBytes());

    const ix = ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: sessionPda, // Session PDA
      newAuthority: newUserPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2,
      authPubkey: newUser.publicKey.toBytes(),
      authorizerSigner: sessionKey.publicKey,
    });

    const result = await tryProcessInstruction(ctx, ix, [sessionKey]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
  }, 30_000);

  it("Success: Secp256r1 Admin creates a session", async () => {
    const { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign, generateAuthenticatorData } = await import("./secp256r1Utils");
    const secpAdmin = await generateMockSecp256r1Signer();
    const [secpAdminPda] = findAuthorityPda(walletPda, secpAdmin.credentialIdHash);

    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: secpAdminPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 1, // Secp256r1
      newRole: 1,  // Admin
      authPubkey: secpAdmin.publicKeyBytes,
      credentialHash: secpAdmin.credentialIdHash,
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    const expiresAt = 999999999n;

    const createSessionIx = ctx.client.createSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: secpAdminPda,
      session: sessionPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      sessionKey: Array.from(sessionKey.publicKey.toBytes()),
      expiresAt,
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
