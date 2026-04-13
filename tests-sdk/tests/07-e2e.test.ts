import { describe, it, expect, beforeAll } from 'vitest';
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import * as crypto from 'crypto';
import { setupTest, sendTx, getSlot, type TestContext } from './common';
import { generateMockSecp256r1Key, signSecp256r1 } from './secp256r1Utils';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  createCreateWalletIx,
  createAddAuthorityIx,
  createRemoveAuthorityIx,
  createTransferOwnershipIx,
  createExecuteIx,
  createCreateSessionIx,
  packCompactInstructions,
  computeAccountsHash,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  ROLE_ADMIN,
  ROLE_SPENDER,
  DISC_ADD_AUTHORITY,
  DISC_EXECUTE,
  DISC_REMOVE_AUTHORITY,
  DISC_TRANSFER_OWNERSHIP,
} from '../../sdk/solita-client/src';
import { AuthorityAccount } from '../../sdk/solita-client/src/generated/accounts';

/**
 * E2E Company Workflow:
 *   1. CEO creates wallet with Secp256r1 passkey
 *   2. CEO adds Admin (Ed25519)
 *   3. Admin adds Spender (Secp256r1)
 *   4. Spender executes SOL transfer
 *   5. Admin creates Session
 *   6. Admin removes Spender
 *   7. CEO transfers ownership to new Secp256r1 key
 */
describe('E2E Company Workflow', () => {
  let ctx: TestContext;

  let ceoKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
  let adminKp: Keypair;
  let spenderKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;

  let walletPda: PublicKey;
  let vaultPda: PublicKey;
  let ceoAuthPda: PublicKey;
  let adminAuthPda: PublicKey;
  let spenderAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();
    ceoKey = await generateMockSecp256r1Key('company.com');
    adminKp = Keypair.generate();
    spenderKey = await generateMockSecp256r1Key('company.com');
  });

  it('Step 1: CEO creates wallet with passkey', async () => {
    const userSeed = crypto.randomBytes(32);

    [walletPda] = findWalletPda(userSeed);
    [vaultPda] = findVaultPda(walletPda);
    const [authPda, authBump] = findAuthorityPda(walletPda, ceoKey.credentialIdHash);
    ceoAuthPda = authPda;

    await sendTx(ctx, [createCreateWalletIx({
      payer: ctx.payer.publicKey,
      walletPda,
      vaultPda,
      authorityPda: ceoAuthPda,
      userSeed,
      authType: AUTH_TYPE_SECP256R1,
      authBump,
      credentialOrPubkey: ceoKey.credentialIdHash,
      secp256r1Pubkey: ceoKey.publicKeyBytes,
      rpId: ceoKey.rpId,
    })]);

    // Fund the vault
    const sig = await ctx.connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL);
    await ctx.connection.confirmTransaction(sig, 'confirmed');

    const auth = await AuthorityAccount.fromAccountAddress(ctx.connection, ceoAuthPda);
    expect(auth.role).toBe(0); // Owner
    expect(auth.authorityType).toBe(AUTH_TYPE_SECP256R1);
  });

  it('Step 2: CEO adds Admin (Ed25519)', async () => {
    const adminPubkey = adminKp.publicKey.toBytes();
    [adminAuthPda] = findAuthorityPda(walletPda, adminPubkey);

    const slot = await getSlot(ctx);
    const dataPayload = Buffer.concat([
      Buffer.from([AUTH_TYPE_ED25519, ROLE_ADMIN]),
      Buffer.alloc(6),
      adminPubkey,
    ]);
    // On-chain extends: extended_data_payload = data_payload + payer.key()
    const signedPayload = Buffer.concat([dataPayload, ctx.payer.publicKey.toBuffer()]);

    const { authPayload, precompileIx } = await signSecp256r1({
      key: ceoKey,
      discriminator: new Uint8Array([DISC_ADD_AUTHORITY]),
      signedPayload,
      slot,
      counter: 1, // CEO's first operation
      payer: ctx.payer.publicKey,
      sysvarIxIndex: 6,
    });

    await sendTx(ctx, [precompileIx, createAddAuthorityIx({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: ceoAuthPda,
      newAuthorityPda: adminAuthPda,
      newType: AUTH_TYPE_ED25519,
      newRole: ROLE_ADMIN,
      credentialOrPubkey: adminPubkey,
      authPayload,
    })]);

    const auth = await AuthorityAccount.fromAccountAddress(ctx.connection, adminAuthPda);
    expect(auth.role).toBe(ROLE_ADMIN);
    expect(auth.authorityType).toBe(AUTH_TYPE_ED25519);
  });

  it('Step 3: Admin adds Spender (Secp256r1)', async () => {
    [spenderAuthPda] = findAuthorityPda(walletPda, spenderKey.credentialIdHash);

    const ix = createAddAuthorityIx({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: adminAuthPda,
      newAuthorityPda: spenderAuthPda,
      newType: AUTH_TYPE_SECP256R1,
      newRole: ROLE_SPENDER,
      credentialOrPubkey: spenderKey.credentialIdHash,
      secp256r1Pubkey: spenderKey.publicKeyBytes,
      rpId: spenderKey.rpId,
      authorizerSigner: adminKp.publicKey,
    });

    await sendTx(ctx, [ix], [adminKp]);

    const auth = await AuthorityAccount.fromAccountAddress(ctx.connection, spenderAuthPda);
    expect(auth.role).toBe(ROLE_SPENDER);
    expect(auth.authorityType).toBe(AUTH_TYPE_SECP256R1);
    expect(Number(auth.counter)).toBe(0);
  });

  it('Step 4: Spender executes SOL transfer', async () => {
    const recipient = Keypair.generate().publicKey;
    const slot = await getSlot(ctx);

    const transferData = Buffer.alloc(12);
    transferData.writeUInt32LE(2, 0);
    transferData.writeBigUInt64LE(1_000_000n, 4);

    const compactIxs = [{
      programIdIndex: 5,
      accountIndexes: [3, 6],
      data: new Uint8Array(transferData),
    }];
    const packed = packCompactInstructions(compactIxs);

    // On-chain extends: signed_payload = compact_bytes + accounts_hash
    const allAccountMetas = [
      { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: false },
      { pubkey: walletPda, isSigner: false, isWritable: false },
      { pubkey: spenderAuthPda, isSigner: false, isWritable: true },
      { pubkey: vaultPda, isSigner: false, isWritable: true },
      { pubkey: PublicKey.default, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: recipient, isSigner: false, isWritable: true },
    ];
    const accountsHash = computeAccountsHash(allAccountMetas, compactIxs);
    const signedPayload = Buffer.concat([packed, accountsHash]);

    const { authPayload, precompileIx } = await signSecp256r1({
      key: spenderKey,
      discriminator: new Uint8Array([DISC_EXECUTE]),
      signedPayload,
      slot,
      counter: 1, // Spender's first op
      payer: ctx.payer.publicKey,
      sysvarIxIndex: 4,
    });

    const ix = createExecuteIx({
      payer: ctx.payer.publicKey,
      walletPda,
      authorityPda: spenderAuthPda,
      vaultPda,
      packedInstructions: packed,
      authPayload,
      remainingAccounts: [
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: recipient, isSigner: false, isWritable: true },
      ],
    });

    const balanceBefore = await ctx.connection.getBalance(recipient);
    await sendTx(ctx, [precompileIx, ix]);
    const balanceAfter = await ctx.connection.getBalance(recipient);

    expect(balanceAfter - balanceBefore).toBe(1_000_000);
  });

  it('Step 5: Admin creates Session', async () => {
    const sessionKp = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKp.publicKey.toBytes());
    // Expires ~1 hour from now in slots (~2.5 slots/sec * 3600 = 9000 slots)
    const currentSlot = await getSlot(ctx);
    const expiresAt = currentSlot + 9000n;

    const ix = createCreateSessionIx({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: adminAuthPda,
      sessionPda,
      sessionKey: sessionKp.publicKey.toBytes(),
      expiresAt,
      authorizerSigner: adminKp.publicKey,
    });

    await sendTx(ctx, [ix], [adminKp]);
  });

  it('Step 6: Admin removes Spender', async () => {
    const ix = createRemoveAuthorityIx({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: adminAuthPda,
      targetAuthorityPda: spenderAuthPda,
      refundDestination: ctx.payer.publicKey,
      authorizerSigner: adminKp.publicKey,
    });

    await sendTx(ctx, [ix], [adminKp]);

    // Verify spender account is closed
    const info = await ctx.connection.getAccountInfo(spenderAuthPda);
    expect(info).toBeNull();
  });

  it('Step 7: CEO transfers ownership to new passkey', async () => {
    const newCeoKey = await generateMockSecp256r1Key('company.com');
    const [newOwnerAuthPda] = findAuthorityPda(walletPda, newCeoKey.credentialIdHash);

    const slot = await getSlot(ctx);
    // On-chain: data_payload = auth_type(1) + credential_id_hash(32) + pubkey(33) + rpIdLen(1) + rpId(N)
    const rpIdBytes = Buffer.from(newCeoKey.rpId, 'utf-8');
    const dataPayload = Buffer.concat([
      Buffer.from([AUTH_TYPE_SECP256R1]),
      newCeoKey.credentialIdHash,
      newCeoKey.publicKeyBytes,
      Buffer.from([rpIdBytes.length]),
      rpIdBytes,
    ]);
    // On-chain extends: extended_data_payload = data_payload + payer.key()
    const signedPayload = Buffer.concat([dataPayload, ctx.payer.publicKey.toBuffer()]);

    const { authPayload, precompileIx } = await signSecp256r1({
      key: ceoKey,
      discriminator: new Uint8Array([DISC_TRANSFER_OWNERSHIP]),
      signedPayload,
      slot,
      counter: 2, // CEO's second operation (1st was AddAuthority)
      payer: ctx.payer.publicKey,
      sysvarIxIndex: 6,
    });

    const ix = createTransferOwnershipIx({
      payer: ctx.payer.publicKey,
      walletPda,
      currentOwnerAuthorityPda: ceoAuthPda,
      newOwnerAuthorityPda: newOwnerAuthPda,
      newType: AUTH_TYPE_SECP256R1,
      credentialOrPubkey: newCeoKey.credentialIdHash,
      secp256r1Pubkey: newCeoKey.publicKeyBytes,
      rpId: newCeoKey.rpId,
      authPayload,
    });

    await sendTx(ctx, [precompileIx, ix]);

    // Old CEO authority should be closed
    const oldInfo = await ctx.connection.getAccountInfo(ceoAuthPda);
    expect(oldInfo).toBeNull();

    // New CEO authority should exist as owner
    const newAuth = await AuthorityAccount.fromAccountAddress(ctx.connection, newOwnerAuthPda);
    expect(newAuth.role).toBe(0); // Owner
    expect(newAuth.authorityType).toBe(AUTH_TYPE_SECP256R1);
    expect(Number(newAuth.counter)).toBe(0); // Fresh counter
  });
});
