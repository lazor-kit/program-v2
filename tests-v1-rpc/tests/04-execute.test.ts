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

    const { ix: ixAdd } = await ctx.highClient.addAuthority({ payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthPubkey: spender.publicKey.toBytes(),
      newAuthType: AuthType.Ed25519,
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
    const { generateMockSecp256r1Signer, buildAuthPayload, buildSecp256r1Message, buildSecp256r1PrecompileIx, buildAuthenticatorData, readCurrentSlot, appendSecp256r1Sysvars } = await import("./secp256r1Utils");
    const { computeAccountsHash } = await import("@lazorkit/solita-client");
    const secpAdmin = await generateMockSecp256r1Signer();
    const [secpAdminPda] = findAuthorityPda(walletPda, secpAdmin.credentialIdHash);

    // Add Secp256r1 Admin
    const { ix: ixAddSecp } = await ctx.highClient.addAuthority({ payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: ownerKeypair,
      newAuthPubkey: secpAdmin.publicKeyBytes,
      newAuthType: AuthType.Secp256r1,
      role: Role.Admin,
      walletPda,
      newCredentialHash: secpAdmin.credentialIdHash
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

    // Append sysvars via SDK helper
    const { ix: ixWithSysvars, sysvarIxIndex, sysvarSlotIndex } = appendSecp256r1Sysvars(executeIx);

    const argsDataExecute = ixWithSysvars.data.subarray(1); // after discriminator

    // Read slot via SDK
    const currentSlot = await readCurrentSlot(ctx.connection);

    // Build auth payload via SDK
    const authenticatorData = await buildAuthenticatorData("example.com");
    const authPayload = buildAuthPayload({
      sysvarIxIndex,
      sysvarSlotIndex,
      authenticatorData,
      slot: currentSlot,
    });

    // Compute accounts hash using SDK function
    // Build compact instructions to match what buildExecute produced
    const compactIxs = [{
      programIdIndex: ixWithSysvars.keys.findIndex(k => k.pubkey.equals(SystemProgram.programId)),
      accountIndexes: [
        ixWithSysvars.keys.findIndex(k => k.pubkey.equals(vaultPda)),
        ixWithSysvars.keys.findIndex(k => k.pubkey.equals(recipient)),
      ],
      data: innerInstructions[0].data,
    }];
    const accountsHash = await computeAccountsHash(ixWithSysvars.keys, compactIxs);

    // signedPayload: compact_instructions + accounts_hash
    const signedPayload = new Uint8Array(argsDataExecute.length + 32);
    signedPayload.set(argsDataExecute, 0);
    signedPayload.set(accountsHash, argsDataExecute.length);

    // Build message via SDK
    const msgToSign = await buildSecp256r1Message({
      discriminator: 4, // Execute
      authPayload,
      signedPayload,
      payer: ctx.payer.publicKey,
      programId: PROGRAM_ID,
      slot: currentSlot,
    });

    // Build precompile instruction via SDK
    const sysvarIx = await buildSecp256r1PrecompileIx(secpAdmin, msgToSign);

    // Pack the payload into executeIx.data
    const finalExecuteData = new Uint8Array(1 + argsDataExecute.length + authPayload.length);
    finalExecuteData[0] = 4; // discriminator
    finalExecuteData.set(argsDataExecute, 1);
    finalExecuteData.set(authPayload, 1 + argsDataExecute.length);
    ixWithSysvars.data = Buffer.from(finalExecuteData);

    const result = await tryProcessInstructions(ctx, [sysvarIx, ixWithSysvars]);
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
