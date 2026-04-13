import { describe, it, expect, beforeAll } from 'vitest';
import {
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import * as crypto from 'crypto';
import { setupTest, sendTx, getSlot, type TestContext } from './common';
import { generateMockSecp256r1Key, createMockSigner } from './secp256r1Utils';
import {
  LazorKitClient,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  ROLE_ADMIN,
  ROLE_SPENDER,
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
  let client: LazorKitClient;

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
    client = new LazorKitClient(ctx.connection);
    ceoKey = await generateMockSecp256r1Key('company.com');
    adminKp = Keypair.generate();
    spenderKey = await generateMockSecp256r1Key('company.com');
  });

  it('Step 1: CEO creates wallet with passkey', async () => {
    const userSeed = crypto.randomBytes(32);

    const result = client.createWalletSecp256r1({
      payer: ctx.payer.publicKey,
      userSeed,
      credentialIdHash: ceoKey.credentialIdHash,
      compressedPubkey: ceoKey.publicKeyBytes,
      rpId: ceoKey.rpId,
    });
    walletPda = result.walletPda;
    vaultPda = result.vaultPda;
    ceoAuthPda = result.authorityPda;

    await sendTx(ctx, [result.ix]);

    // Fund the vault
    const sig = await ctx.connection.requestAirdrop(
      vaultPda,
      5 * LAMPORTS_PER_SOL,
    );
    await ctx.connection.confirmTransaction(sig, 'confirmed');

    const auth = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      ceoAuthPda,
    );
    expect(auth.role).toBe(0); // Owner
    expect(auth.authorityType).toBe(AUTH_TYPE_SECP256R1);
  });

  it('Step 2: CEO adds Admin (Ed25519)', async () => {
    const ceoSigner = createMockSigner(ceoKey);

    const { ix, newAuthorityPda, precompileIx } =
      await client.addAuthoritySecp256r1({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ceoAuthPda,
        adminSigner: ceoSigner,
        newType: AUTH_TYPE_ED25519,
        newRole: ROLE_ADMIN,
        newCredentialOrPubkey: adminKp.publicKey.toBytes(),
      });
    adminAuthPda = newAuthorityPda;

    await sendTx(ctx, [precompileIx, ix]);

    const auth = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      adminAuthPda,
    );
    expect(auth.role).toBe(ROLE_ADMIN);
    expect(auth.authorityType).toBe(AUTH_TYPE_ED25519);
  });

  it('Step 3: Admin adds Spender (Secp256r1)', async () => {
    const { ix, newAuthorityPda } = client.addAuthorityEd25519({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: adminAuthPda,
      adminSigner: adminKp.publicKey,
      newType: AUTH_TYPE_SECP256R1,
      newRole: ROLE_SPENDER,
      newCredentialOrPubkey: spenderKey.credentialIdHash,
      newSecp256r1Pubkey: spenderKey.publicKeyBytes,
      newRpId: spenderKey.rpId,
    });
    spenderAuthPda = newAuthorityPda;

    await sendTx(ctx, [ix], [adminKp]);

    const auth = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      spenderAuthPda,
    );
    expect(auth.role).toBe(ROLE_SPENDER);
    expect(auth.authorityType).toBe(AUTH_TYPE_SECP256R1);
    expect(Number(auth.counter)).toBe(0);
  });

  it('Step 4: Spender executes SOL transfer', async () => {
    const recipient = Keypair.generate().publicKey;
    const spenderSigner = createMockSigner(spenderKey);

    const ixs = await client.transferSol({
      payer: ctx.payer.publicKey,
      walletPda,
      signer: spenderSigner,
      recipient,
      lamports: 1_000_000n,
    });

    const balanceBefore = await ctx.connection.getBalance(recipient);
    await sendTx(ctx, ixs);
    const balanceAfter = await ctx.connection.getBalance(recipient);

    expect(balanceAfter - balanceBefore).toBe(1_000_000);
  });

  it('Step 5: Admin creates Session', async () => {
    const sessionKp = Keypair.generate();
    const currentSlot = await getSlot(ctx);
    const expiresAt = currentSlot + 9000n;

    const { ix, sessionPda } = client.createSessionEd25519({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: adminAuthPda,
      adminSigner: adminKp.publicKey,
      sessionKey: sessionKp.publicKey.toBytes(),
      expiresAt,
    });

    await sendTx(ctx, [ix], [adminKp]);
  });

  it('Step 6: Admin removes Spender', async () => {
    const ix = client.removeAuthorityEd25519({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: adminAuthPda,
      adminSigner: adminKp.publicKey,
      targetAuthorityPda: spenderAuthPda,
    });

    await sendTx(ctx, [ix], [adminKp]);

    // Verify spender account is closed
    const info = await ctx.connection.getAccountInfo(spenderAuthPda);
    expect(info).toBeNull();
  });

  it('Step 7: CEO transfers ownership to new passkey', async () => {
    const newCeoKey = await generateMockSecp256r1Key('company.com');
    const ceoSigner = createMockSigner(ceoKey);

    const { ix, newOwnerAuthorityPda, precompileIx } =
      await client.transferOwnershipSecp256r1({
        payer: ctx.payer.publicKey,
        walletPda,
        currentOwnerAuthorityPda: ceoAuthPda,
        ownerSigner: ceoSigner,
        newType: AUTH_TYPE_SECP256R1,
        newCredentialOrPubkey: newCeoKey.credentialIdHash,
        newSecp256r1Pubkey: newCeoKey.publicKeyBytes,
        newRpId: newCeoKey.rpId,
      });

    await sendTx(ctx, [precompileIx, ix]);

    // Old CEO authority should be closed
    const oldInfo = await ctx.connection.getAccountInfo(ceoAuthPda);
    expect(oldInfo).toBeNull();

    // New CEO authority should exist as owner
    const newAuth = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      newOwnerAuthorityPda,
    );
    expect(newAuth.role).toBe(0); // Owner
    expect(newAuth.authorityType).toBe(AUTH_TYPE_SECP256R1);
    expect(Number(newAuth.counter)).toBe(0); // Fresh counter
  });
});
