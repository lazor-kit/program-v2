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

    // Fund vault
    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPda, 200_000_000n)]);
    console.log("Wallet created and funded for execute tests");
  }, 30_000);

  it("Success: Owner executes a transfer", async () => {
    const recipient = Keypair.generate().publicKey;

    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: ownerAuthPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 1_000_000n)
      ],
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [executeIx], [ownerKeypair]);

    const balance = await ctx.connection.getBalance(recipient);
    expect(balance).toBe(1_000_000);
  }, 30_000);

  it("Success: Spender executes a transfer", async () => {
    const spender = Keypair.generate();
    const [spenderPda] = findAuthorityPda(walletPda, spender.publicKey.toBytes());

    await sendTx(ctx, [ctx.client.addAuthority({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      newAuthority: spenderPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      authType: 0, newRole: 2, // Spender
      authPubkey: spender.publicKey.toBytes(),
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    const recipient = Keypair.generate().publicKey;

    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: spenderPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 1_000_000n)
      ],
      authorizerSigner: spender.publicKey,
    });

    await sendTx(ctx, [executeIx], [spender]);

    const balance = await ctx.connection.getBalance(recipient);
    expect(balance).toBe(1_000_000);
  }, 30_000);

  it("Success: Session key executes a transfer", async () => {
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

    const recipient = Keypair.generate().publicKey;

    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: sessionPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 1_000_000n)
      ],
      authorizerSigner: sessionKey.publicKey,
    });

    executeIx.keys.forEach(k => {
      if (k.pubkey.equals(sessionPda)) k.isSigner = false; // builder adds it as isSigner sometimes if matches?
    });

    await sendTx(ctx, [executeIx], [sessionKey]);

    const balance = await ctx.connection.getBalance(recipient);
    expect(balance).toBe(1_000_000);
  }, 30_000);

  it("Success: Secp256r1 Admin executes a transfer", async () => {
    const { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign } = await import("./secp256r1Utils");
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

    const recipient = Keypair.generate().publicKey;
    const innerInstructions = [
      getSystemTransferIx(vaultPda, recipient, 2_000_000n)
    ];

    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: secpAdminPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
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
    await sendTx(ctx, [ctx.client.createSession({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      session: sessionPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      sessionKey: Array.from(sessionKey.publicKey.toBytes()),
      expiresAt: 0n,
      authorizerSigner: ownerKeypair.publicKey,
    })], [ownerKeypair]);

    const recipient = Keypair.generate().publicKey;
    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: sessionPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 100n)
      ],
      authorizerSigner: sessionKey.publicKey,
    });

    const result = await tryProcessInstruction(ctx, executeIx, [sessionKey]);
    expect(result.result).toMatch(/3009|0xbc1|simulation failed/i);
  }, 30_000);

  it("Failure: Unauthorized signatory", async () => {
    const thief = Keypair.generate();
    const recipient = Keypair.generate().publicKey;

    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: ownerAuthPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient, 100n)
      ],
      authorizerSigner: thief.publicKey,
    });

    const result = await tryProcessInstruction(ctx, executeIx, [thief]);
    expect(result.result).toMatch(/signature|unauthorized|simulation failed/i);
  }, 30_000);

  it("Failure: Authority from Wallet A cannot execute on Wallet B's vault", async () => {
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

    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPdaB, 100_000_000n)]);

    const recipient = Keypair.generate().publicKey;

    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPdaB,         // Target B
      authority: ownerAuthPda,     // Auth A
      vault: vaultPdaB,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [
        getSystemTransferIx(vaultPdaB, recipient, 1_000_000n)
      ],
      authorizerSigner: ownerKeypair.publicKey,
    });

    const result = await tryProcessInstruction(ctx, executeIx, [ownerKeypair]);
    expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
  }, 30_000);

  it("Success: Execute batch — multiple transfers", async () => {
    const recipient1 = Keypair.generate().publicKey;
    const recipient2 = Keypair.generate().publicKey;

    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: ownerAuthPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [
        getSystemTransferIx(vaultPda, recipient1, 1_000_000n),
        getSystemTransferIx(vaultPda, recipient2, 2_000_000n),
      ],
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [executeIx], [ownerKeypair]);

    const bal1 = await ctx.connection.getBalance(recipient1);
    const bal2 = await ctx.connection.getBalance(recipient2);
    expect(bal1).toBe(1_000_000);
    expect(bal2).toBe(2_000_000);
  }, 30_000);

  it("Success: Execute with empty inner instructions", async () => {
    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: ownerAuthPda,
      vault: vaultPda,
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [],
      authorizerSigner: ownerKeypair.publicKey,
    });

    await sendTx(ctx, [executeIx], [ownerKeypair]);
  }, 30_000);

  it("Failure: Execute with wrong vault PDA", async () => {
    const fakeVault = Keypair.generate();
    const recipient = Keypair.generate().publicKey;

    const executeIx = ctx.client.buildExecute({
      payer: ctx.payer.publicKey,
      wallet: walletPda,
      authority: ownerAuthPda,
      vault: fakeVault.publicKey, // Fake
      config: ctx.configPda,
      treasuryShard: ctx.treasuryShard,
      innerInstructions: [
        getSystemTransferIx(fakeVault.publicKey, recipient, 1_000_000n)
      ],
      authorizerSigner: ownerKeypair.publicKey,
    });

    const result = await tryProcessInstruction(ctx, executeIx, [ownerKeypair, fakeVault]);
    expect(result.result).toMatch(/simulation failed|InvalidSeeds/i);
  }, 30_000);
});
