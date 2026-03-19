/**
 * LazorKit V1 Client — Execute tests
 *
 * Tests: Execute instruction with SOL transfer inner instructions, batches, re-entrancy, and Secp256r1.
 */

import { Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  LazorClient,
  AuthType,
  Role // <--- Add AuthType, Role
} from "@lazorkit/solita-client";
import { setupTest, sendTx, tryProcessInstruction, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";

function getRandomSeed() {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  return seed;
}

describe("LazorKit V1 — Execute", () => {
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

    const { ix, walletPda: w, authorityPda } = await ctx.highClient.createWallet({
          payer: ctx.payer,
          authType: AuthType.Ed25519,
          owner: ownerKeypair.publicKey,
    });
    await sendTx(ctx, [ix]);
    walletPda = w;
    ownerAuthPda = authorityPda;
 
    const [v] = findVaultPda(walletPda);
    vaultPda = v;
 
    // Fund vault
    const [vPda] = findVaultPda(walletPda);
    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vPda, 200_000_000n)]);
    console.log("Wallet created and funded for execute tests");
  }, 30_000);

  it("Success: Owner executes a transfer", async () => {
    const recipient = Keypair.generate().publicKey;

    const ix = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: ownerAuthPda,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 1_000_000n)
      ],
      signer: ownerKeypair
    });
    await sendTx(ctx, [ix], [ownerKeypair]);

    const balance = await ctx.connection.getBalance(recipient);
    expect(balance).toBe(1_000_000);
  }, 30_000);

  it("Success: Spender executes a transfer", async () => {
    const spender = Keypair.generate();
    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());

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

    const recipient = Keypair.generate().publicKey;

    // Execute using spender key
    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: spenderPda,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 1_000_000n)
      ],
      signer: spender
    });
    await sendTx(ctx, [executeIx], [spender]);

    const balance = await ctx.connection.getBalance(recipient);
    expect(balance).toBe(1_000_000);
  }, 30_000);

  it("Success: Session key executes a transfer", async () => {
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

  it("Success: Secp256r1 Admin executes a transfer", async () => {
    const { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign } = await import("./secp256r1Utils");
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

    const recipient = Keypair.generate().publicKey;
    const innerInstructions = [
      getSystemTransferIx(vaultPda, recipient, 2_000_000n)
    ];

    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: secpAdminPda,
      innerInstructions,
    });

    executeIx.keys = [
      ...(executeIx.keys || []),
      { pubkey: new PublicKey("Sysvar1nstructions1111111111111111111111111"), isSigner: false, isWritable: false },
      { pubkey: new PublicKey("SysvarS1otHashes111111111111111111111111111"), isSigner: false, isWritable: false }
    ];

    const argsDataExecute = executeIx.data.subarray(1); // after discriminator

    const slotHashesAddress = new PublicKey("SysvarS1otHashes111111111111111111111111111");
    const accountInfo = await ctx.connection.getAccountInfo(slotHashesAddress);
    const rawData = accountInfo!.data;
    const currentSlot = new DataView(rawData.buffer, rawData.byteOffset, rawData.byteLength).getBigUint64(8, true);

    const sysvarIxIndex = executeIx.keys.length - 2;
    const sysvarSlotIndex = executeIx.keys.length - 1;

    const { generateAuthenticatorData } = await import("./secp256r1Utils");
    const authenticatorDataRaw = generateAuthenticatorData("example.com");

    const authPayload = buildSecp256r1AuthPayload(sysvarIxIndex, sysvarSlotIndex, authenticatorDataRaw, currentSlot);

    // Compute Accounts Hash
    const systemProgramId = SystemProgram.programId;
    const accountsHashData = new Uint8Array(32 * 3);
    accountsHashData.set(systemProgramId.toBytes(), 0);
    accountsHashData.set(vaultPda.toBytes(), 32);
    accountsHashData.set(recipient.toBytes(), 64);

    const crypto = await import("crypto");
    const accountsHashHasher = crypto.createHash('sha256');
    accountsHashHasher.update(accountsHashData);
    const accountsHash = new Uint8Array(accountsHashHasher.digest());

    // signedPayload: compact_instructions + accounts_hash
    const signedPayload = new Uint8Array(argsDataExecute.length + 32);
    signedPayload.set(argsDataExecute, 0);
    signedPayload.set(accountsHash, argsDataExecute.length);

    const currentSlotBytes = new Uint8Array(8);
    new DataView(currentSlotBytes.buffer).setBigUint64(0, currentSlot, true);

    const discriminator = new Uint8Array([4]); // Execute
    const msgToSign = getSecp256r1MessageToSign(
      discriminator,
      authPayload,
      signedPayload,
      ctx.payer.publicKey.toBytes(),
      PROGRAM_ID.toBytes(),
      authenticatorDataRaw,
      currentSlotBytes
    );

    const sysvarIx = await createSecp256r1Instruction(secpAdmin, msgToSign);

    // Pack the payload into executeIx.data
    const finalExecuteData = new Uint8Array(1 + argsDataExecute.length + authPayload.length);
    finalExecuteData.set(discriminator, 0);
    finalExecuteData.set(argsDataExecute, 1);
    finalExecuteData.set(authPayload, 1 + argsDataExecute.length);
    executeIx.data = Buffer.from(finalExecuteData);

    const result = await tryProcessInstructions(ctx, [sysvarIx, executeIx]);
    expect(result.result).toBe("ok");

    const balance = await ctx.connection.getBalance(recipient);
    expect(balance).toBe(2_000_000);
  }, 30_000);

  it("Failure: Session expired", async () => {
    const sessionKey = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

    // Create session that is immediately expired (slot 0 or far past)
    const { ix: ixCreate } = await ctx.highClient.createSession({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      sessionKey: sessionKey.publicKey,
      expiresAt: 0n,
      walletPda
    });
    await sendTx(ctx, [ixCreate], [ownerKeypair]);

    const recipient = Keypair.generate().publicKey;
    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: sessionPda,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 100n)
      ],
      signer: sessionKey
    });

    const result = await tryProcessInstruction(ctx, [executeIx], [sessionKey]);
    expect(result.result).toMatch(/3009|0xbc1|simulation failed/i);
  }, 30_000);

  it("Failure: Unauthorized signatory", async () => {
    const thief = Keypair.generate();
    const recipient = Keypair.generate().publicKey;

    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: ownerAuthPda,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 100n)
      ],
      signer: thief
    });

    const result = await tryProcessInstruction(ctx, [executeIx], [thief]);
    expect(result.result).toMatch(/signature|unauthorized|simulation failed/i);
  }, 30_000);

  it("Failure: Authority from Wallet A cannot execute on Wallet B's vault", async () => {
    const userSeedB = getRandomSeed();
    const [walletPdaB] = findWalletPda(userSeedB);
    const [vaultPdaB] = findVaultPda(walletPdaB);
    const ownerB = Keypair.generate();
    const [ownerBAuthPda] = findAuthorityPda(walletPdaB, ownerB.publicKey.toBytes());

    const { ix: ixCreateB } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: ownerB.publicKey,
      userSeed: userSeedB
    });
    await sendTx(ctx, [ixCreateB]);

    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPdaB, 100_000_000n)]);

    const recipient = Keypair.generate().publicKey;

    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda: walletPdaB,         // Target B
      authorityPda: ownerAuthPda,     // Auth A
      innerInstructions: [
        getSystemTransferIx(vaultPdaB, recipient, 1_000_000n)
      ],
      signer: ownerKeypair
    });

    const result = await tryProcessInstruction(ctx, [executeIx], [ownerKeypair]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
  }, 30_000);

  it("Success: Execute batch — multiple transfers", async () => {
    const recipient1 = Keypair.generate().publicKey;
    const recipient2 = Keypair.generate().publicKey;

    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: ownerAuthPda,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient1, 1_000_000n),
        getSystemTransferIx(vaultPda, recipient2, 2_000_000n),
      ],
      signer: ownerKeypair
    });

    await sendTx(ctx, [executeIx], [ownerKeypair]);

    const bal1 = await ctx.connection.getBalance(recipient1);
    const bal2 = await ctx.connection.getBalance(recipient2);
    expect(bal1).toBe(1_000_000);
    expect(bal2).toBe(2_000_000);
  }, 30_000);

  it("Success: Execute with empty inner instructions", async () => {
    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: ownerAuthPda,
      innerInstructions: [],
      signer: ownerKeypair
    });

    await sendTx(ctx, [executeIx], [ownerKeypair]);
  }, 30_000);

  it("Failure: Execute with wrong vault PDA", async () => {
    const fakeVault = Keypair.generate();
    const recipient = Keypair.generate().publicKey;

    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      authorityPda: ownerAuthPda,
      innerInstructions: [
        getSystemTransferIx(fakeVault.publicKey, recipient, 1_000_000n)
      ],
      signer: ownerKeypair,
      vaultPda: fakeVault.publicKey
    });

    const result = await tryProcessInstruction(ctx, [executeIx], [ownerKeypair, fakeVault]);
    expect(result.result).toMatch(/simulation failed|InvalidSeeds/i);
  }, 30_000);
});
