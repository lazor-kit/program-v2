/**
 * 02-wallet.test.ts
 *
 * Tests for: CreateWallet, Wallet Discovery, Account Data Integrity, LazorClient wrapper (create/execute/txn)
 * Merged from: wallet.test.ts (create + discovery), discovery.test.ts, full_flow.test.ts, integrity.test.ts, high_level.test.ts
 */

import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  AuthorityAccount,
  LazorClient,
  AuthType,
  Role,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, getRandomSeed, tryProcessInstruction, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";

describe("CreateWallet & Discovery", () => {
  let ctx: TestContext;

  beforeAll(async () => {
    ctx = await setupTest();
  }, 30_000);

  // ─── Create Wallet ─────────────────────────────────────────────────────────

  it("Create wallet with Ed25519 owner", async () => {
    const userSeed = getRandomSeed();
    const owner = Keypair.generate();

    const { ix, walletPda } = await ctx.highClient.createWallet({
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

  it("Create wallet with Secp256r1 (WebAuthn) owner", async () => {
    const userSeed = getRandomSeed();
    const credentialIdHash = getRandomSeed();
    const p256Pubkey = new Uint8Array(33).map(() => Math.floor(Math.random() * 256));
    p256Pubkey[0] = 0x02;

    const { ix, walletPda } = await ctx.highClient.createWallet({
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

  it("Create wallet with Web Crypto P-256 keypair (real RPC)", async () => {
    const userSeed = new Uint8Array(32);
    crypto.getRandomValues(userSeed);

    const [walletPda] = findWalletPda(userSeed);

    const p256Keypair = await crypto.subtle.generateKey(
      { name: "ECDSA", namedCurve: "P-256" },
      true,
      ["sign", "verify"]
    );

    const rpId = "lazorkit.valid";
    const rpIdHashBuffer = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(rpId));
    const credentialIdHash = new Uint8Array(rpIdHashBuffer);

    const spki = await crypto.subtle.exportKey("spki", p256Keypair.publicKey);
    let rawP256Pubkey = new Uint8Array(spki).slice(-64);
    let p256PubkeyCompressed = new Uint8Array(33);
    p256PubkeyCompressed[0] = (rawP256Pubkey[63] % 2 === 0) ? 0x02 : 0x03;
    p256PubkeyCompressed.set(rawP256Pubkey.slice(0, 32), 1);

    const { ix } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Secp256r1,
      pubkey: p256PubkeyCompressed,
      credentialHash: credentialIdHash,
      userSeed
    });

    const txResult = await sendTx(ctx, [ix]);
    expect(txResult).toBeDefined();

    const res = await ctx.connection.getAccountInfo(walletPda);
    expect(res).toBeDefined();
    expect(res!.data).toBeDefined();
  }, 30_000);

  it("Failure: Cannot create wallet with same seed twice", async () => {
    const userSeed = getRandomSeed();
    const o = Keypair.generate();
    const { ix: createIx } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o.publicKey,
      userSeed
    });
    await sendTx(ctx, [createIx]);

    const o2 = Keypair.generate();
    const { ix: create2Ix } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: o2.publicKey,
      userSeed
    });

    const result = await tryProcessInstruction(ctx, [create2Ix]);
    expect(result.result).toMatch(/simulation failed|already in use|AccountAlreadyInitialized/i);
  }, 30_000);

  // ─── Discovery ─────────────────────────────────────────────────────────────

  it("Discovery: Ed25519 — pubkey → PDA → wallet", async () => {
    const userSeed = getRandomSeed();
    const owner = Keypair.generate();

    const { ix, walletPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const discoveredWallets = await LazorClient.findWalletsByEd25519Pubkey(ctx.connection, owner.publicKey);
    expect(discoveredWallets).toContainEqual(walletPda);
  }, 30_000);

  it("Discovery: Secp256r1 — credential hash → authorities", async () => {
    const userSeed = new Uint8Array(32);
    crypto.getRandomValues(userSeed);
    const credentialIdHash = new Uint8Array(32);
    crypto.getRandomValues(credentialIdHash);
    const [walletPda] = findWalletPda(userSeed);
    const [authPda] = findAuthorityPda(walletPda, credentialIdHash);

    const authPubkey = new Uint8Array(33).fill(7);

    const { ix: createIx } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Secp256r1,
      pubkey: authPubkey,
      credentialHash: credentialIdHash,
      userSeed
    });
    await sendTx(ctx, [createIx]);

    // Use SDK's findAllAuthoritiesByCredentialHash
    const discovered = await LazorClient.findAllAuthoritiesByCredentialHash(ctx.connection, credentialIdHash);

    expect(discovered.length).toBeGreaterThanOrEqual(1);
    const found = discovered.find((d: any) => d.authority.equals(authPda));
    expect(found).toBeDefined();
    expect(found?.wallet.equals(walletPda)).toBe(true);
    expect(found?.role).toBe(0); // Owner
    expect(found?.authorityType).toBe(1); // Secp256r1
  }, 60_000);

  // ─── Data Integrity ────────────────────────────────────────────────────────

  const HEADER_SIZE = 48;
  const DATA_OFFSET = HEADER_SIZE;
  const SECP256R1_PUBKEY_OFFSET = DATA_OFFSET + 32;

  async function getRawAccountData(ctx: TestContext, address: PublicKey): Promise<Buffer> {
    const acc = await ctx.connection.getAccountInfo(address);
    if (!acc) throw new Error(`Account ${address.toBase58()} not found`);
    return acc.data;
  }

  it("Integrity: Ed25519 pubkey stored at correct offset", async () => {
    const userSeed = getRandomSeed();
    const owner = Keypair.generate();

    const { ix, walletPda, authorityPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const data = await getRawAccountData(ctx, authorityPda);

    expect(data[0]).toBe(2);  // discriminator = Authority
    expect(data[1]).toBe(0);  // authority_type = Ed25519
    expect(data[2]).toBe(0);  // role = Owner

    const storedWallet = data.subarray(16, 48);
    expect(Uint8Array.from(storedWallet)).toEqual(walletPda.toBytes());

    const storedPubkey = data.subarray(DATA_OFFSET, DATA_OFFSET + 32);
    expect(Uint8Array.from(storedPubkey)).toEqual(owner.publicKey.toBytes());
  });

  it("Integrity: Secp256r1 credential_id_hash + pubkey stored at correct offsets", async () => {
    const userSeed = getRandomSeed();
    const credentialIdHash = getRandomSeed();
    const p256Pubkey = new Uint8Array(33);
    p256Pubkey[0] = 0x02;
    crypto.getRandomValues(p256Pubkey.subarray(1));

    const { ix, walletPda, authorityPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Secp256r1,
      pubkey: p256Pubkey,
      credentialHash: credentialIdHash,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const data = await getRawAccountData(ctx, authorityPda);

    expect(data[0]).toBe(2);
    expect(data[1]).toBe(1);
    expect(data[2]).toBe(0);

    const storedCredHash = data.subarray(DATA_OFFSET, DATA_OFFSET + 32);
    expect(Uint8Array.from(storedCredHash)).toEqual(credentialIdHash);

    const storedPubkey = data.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33);
    expect(Uint8Array.from(storedPubkey)).toEqual(p256Pubkey);
  });

  it("Integrity: Multiple Secp256r1 authorities with different credential_id_hash", async () => {
    const userSeed = getRandomSeed();
    const owner = Keypair.generate();

    const { ix, walletPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed
    });
    await sendTx(ctx, [ix]);

    const credHash1 = getRandomSeed();
    const pubkey1 = new Uint8Array(33); pubkey1[0] = 0x02; crypto.getRandomValues(pubkey1.subarray(1));

    const { ix: ixAdd1, newAuthority: authPda1 } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      walletPda,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      newAuthPubkey: pubkey1,
      newAuthType: AuthType.Secp256r1,
      role: Role.Admin,
      newCredentialHash: credHash1
    });
    await sendTx(ctx, [ixAdd1], [owner]);

    const credHash2 = getRandomSeed();
    const pubkey2 = new Uint8Array(33); pubkey2[0] = 0x03; crypto.getRandomValues(pubkey2.subarray(1));

    const { ix: ixAdd2, newAuthority: authPda2 } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      walletPda,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      newAuthPubkey: pubkey2,
      newAuthType: AuthType.Secp256r1,
      role: Role.Spender,
      newCredentialHash: credHash2
    });
    await sendTx(ctx, [ixAdd2], [owner]);

    expect(authPda1.toBase58()).not.toEqual(authPda2.toBase58());

    const data1 = await getRawAccountData(ctx, authPda1);
    expect(data1[1]).toBe(1);
    expect(data1[2]).toBe(1);
    expect(Uint8Array.from(data1.subarray(DATA_OFFSET, DATA_OFFSET + 32))).toEqual(credHash1);

    const data2 = await getRawAccountData(ctx, authPda2);
    expect(data2[1]).toBe(1);
    expect(data2[2]).toBe(2);
    expect(Uint8Array.from(data2.subarray(DATA_OFFSET, DATA_OFFSET + 32))).toEqual(credHash2);
  });

  // ─── LazorClient Wrapper ──────────────────────────────────────────────────

  it("LazorClient: create wallet + execute with simplified APIs", async () => {
    const owner = Keypair.generate();

    const { ix, walletPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey
    });
    await sendTx(ctx, [ix]);
    expect(walletPda).toBeDefined();

    const [vaultPda] = findVaultPda(walletPda);
    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPda, 10_000_000n)]);

    const recipient = Keypair.generate().publicKey;

    const executeIx = await ctx.highClient.execute({
      payer: ctx.payer,
      walletPda,
      innerInstructions: [getSystemTransferIx(vaultPda, recipient, 1_000_000n)],
      signer: owner
    });
    await sendTx(ctx, [executeIx], [owner]);

    const bal = await ctx.connection.getBalance(recipient);
    expect(bal).toBe(1_000_000);
  });

  it("LazorClient: add authority using high-level methods", async () => {
    const owner = Keypair.generate();

    const { ix, walletPda } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey
    });
    await sendTx(ctx, [ix]);

    const newAuthority = Keypair.generate();

    const { ix: ixAdd } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      walletPda,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      newAuthPubkey: newAuthority.publicKey.toBytes(),
      newAuthType: AuthType.Ed25519,
      role: Role.Admin,
    });
    await sendTx(ctx, [ixAdd], [owner]);

    const [newAuthPda] = findAuthorityPda(walletPda, newAuthority.publicKey.toBytes());
    const accInfo = await ctx.connection.getAccountInfo(newAuthPda);
    expect(accInfo).toBeDefined();
    expect(accInfo!.data[0]).toBe(2);
  });

  it("LazorClient: create wallet + execute via Transaction Builders (...Txn)", async () => {
    const owner = Keypair.generate();

    const { transaction, walletPda, authorityPda } = await ctx.highClient.createWalletTxn({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey
    });
    await sendTx(ctx, transaction.instructions);
    expect(walletPda).toBeDefined();

    const [vaultPda] = findVaultPda(walletPda);
    await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPda, 10_000_000n)]);

    const recipient = Keypair.generate().publicKey;

    const execTx = await ctx.highClient.executeTxn({
      payer: ctx.payer,
      walletPda,
      innerInstructions: [getSystemTransferIx(vaultPda, recipient, 1_000_000n)],
      signer: owner
    });
    await sendTx(ctx, execTx.instructions, [owner]);

    const bal = await ctx.connection.getBalance(recipient);
    expect(bal).toBe(1_000_000);
  });
});
