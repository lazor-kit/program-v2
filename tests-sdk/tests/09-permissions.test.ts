/**
 * Permission boundary tests — verifies on-chain role enforcement.
 *
 * Tests that the program correctly prevents:
 * - Spender adding authorities
 * - Spender creating sessions
 * - Admin removing owner
 * - Admin adding admin (admin can only add spender)
 * - Self-removal
 */
import { describe, it, expect, beforeAll } from 'vitest';
import { Keypair, LAMPORTS_PER_SOL, PublicKey } from '@solana/web3.js';
import * as crypto from 'crypto';
import {
  setupTest,
  sendTx,
  sendTxExpectError,
  getSlot,
  type TestContext,
} from './common';
import { generateMockSecp256r1Key, createMockSigner } from './secp256r1Utils';
import {
  LazorKitClient,
  AUTH_TYPE_ED25519,
  ROLE_ADMIN,
  ROLE_SPENDER,
  ed25519,
  secp256r1,
} from '@lazorkit/sdk-legacy';

describe('Permission Boundaries', () => {
  let ctx: TestContext;
  let client: LazorKitClient;

  let walletPda: PublicKey;
  let ownerKp: Keypair;
  let ownerAuthPda: PublicKey;

  let adminKp: Keypair;
  let adminAuthPda: PublicKey;

  let spenderKp: Keypair;
  let spenderAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();
    client = new LazorKitClient(ctx.connection);

    // Create wallet with Ed25519 owner
    ownerKp = Keypair.generate();
    const userSeed = crypto.randomBytes(32);

    const walletResult = await client.createWallet({
      payer: ctx.payer.publicKey,
      userSeed,
      owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
    });
    walletPda = walletResult.walletPda;
    ownerAuthPda = walletResult.authorityPda;
    await sendTx(ctx, walletResult.instructions);

    // Owner adds Admin
    adminKp = Keypair.generate();
    const addAdminResult = await client.addAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      newAuthority: { type: 'ed25519', publicKey: adminKp.publicKey },
      role: ROLE_ADMIN,
    });
    adminAuthPda = addAdminResult.newAuthorityPda;
    await sendTx(ctx, addAdminResult.instructions, [ownerKp]);

    // Admin adds Spender
    spenderKp = Keypair.generate();
    const addSpenderResult = await client.addAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(adminKp.publicKey, adminAuthPda),
      newAuthority: { type: 'ed25519', publicKey: spenderKp.publicKey },
      role: ROLE_SPENDER,
    });
    spenderAuthPda = addSpenderResult.newAuthorityPda;
    await sendTx(ctx, addSpenderResult.instructions, [adminKp]);
  });

  // ─── AddAuthority permission boundaries ─────────────────────────

  it('spender cannot add authority', async () => {
    const newKp = Keypair.generate();

    const { instructions } = await client.addAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(spenderKp.publicKey, spenderAuthPda),
      newAuthority: { type: 'ed25519', publicKey: newKp.publicKey },
      role: ROLE_SPENDER,
    });

    // Error 3002 = PermissionDenied
    await sendTxExpectError(ctx, instructions, [spenderKp], 3002);
  });

  it('admin cannot add admin (only spender)', async () => {
    const newKp = Keypair.generate();

    const { instructions } = await client.addAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(adminKp.publicKey, adminAuthPda),
      newAuthority: { type: 'ed25519', publicKey: newKp.publicKey },
      role: ROLE_ADMIN,
    });

    // Admin can only add Spender, not Admin — PermissionDenied
    await sendTxExpectError(ctx, instructions, [adminKp], 3002);
  });

  it('owner can add admin', async () => {
    const newKp = Keypair.generate();

    const { instructions, newAuthorityPda } = await client.addAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      newAuthority: { type: 'ed25519', publicKey: newKp.publicKey },
      role: ROLE_ADMIN,
    });

    // Owner can add any role — should succeed
    await sendTx(ctx, instructions, [ownerKp]);
  });

  // ─── RemoveAuthority permission boundaries ──────────────────────

  it('admin cannot remove owner', async () => {
    const { instructions } = await client.removeAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(adminKp.publicKey, adminAuthPda),
      targetAuthorityPda: ownerAuthPda,
    });

    // Error 3002 = PermissionDenied (owner cannot be removed)
    await sendTxExpectError(ctx, instructions, [adminKp], 3002);
  });

  it('admin cannot remove another admin', async () => {
    // Create a second admin
    const admin2Kp = Keypair.generate();
    const addResult = await client.addAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      newAuthority: { type: 'ed25519', publicKey: admin2Kp.publicKey },
      role: ROLE_ADMIN,
    });
    await sendTx(ctx, addResult.instructions, [ownerKp]);

    // Admin tries to remove another admin — should fail
    const { instructions } = await client.removeAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(adminKp.publicKey, adminAuthPda),
      targetAuthorityPda: addResult.newAuthorityPda,
    });

    await sendTxExpectError(ctx, instructions, [adminKp], 3002);

    // Clean up — owner removes admin2
    const cleanupResult = await client.removeAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      targetAuthorityPda: addResult.newAuthorityPda,
    });
    await sendTx(ctx, cleanupResult.instructions, [ownerKp]);
  });

  it('admin cannot self-remove', async () => {
    const { instructions } = await client.removeAuthority({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(adminKp.publicKey, adminAuthPda),
      targetAuthorityPda: adminAuthPda,
    });

    // Self-removal is blocked — PermissionDenied
    await sendTxExpectError(ctx, instructions, [adminKp], 3002);
  });

  // ─── CreateSession permission boundaries ────────────────────────

  it('spender cannot create session', async () => {
    const sessionKp = Keypair.generate();
    const currentSlot = await getSlot(ctx);

    const { instructions } = await client.createSession({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(spenderKp.publicKey, spenderAuthPda),
      sessionKey: sessionKp.publicKey,
      expiresAt: currentSlot + 9000n,
    });

    // Error 3002 = PermissionDenied
    await sendTxExpectError(ctx, instructions, [spenderKp], 3002);
  });

  // ─── Secp256r1 permission boundaries ────────────────────────────

  describe('Secp256r1 spender boundaries', () => {
    let secpWalletPda: PublicKey;
    let secpOwnerKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
    let secpOwnerAuthPda: PublicKey;
    let secpSpenderKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
    let secpSpenderAuthPda: PublicKey;

    beforeAll(async () => {
      secpOwnerKey = await generateMockSecp256r1Key();
      const userSeed = crypto.randomBytes(32);

      const result = await client.createWallet({
        payer: ctx.payer.publicKey,
        userSeed,
        owner: {
          type: 'secp256r1',
          credentialIdHash: secpOwnerKey.credentialIdHash,
          compressedPubkey: secpOwnerKey.publicKeyBytes,
          rpId: secpOwnerKey.rpId,
        },
      });
      secpWalletPda = result.walletPda;
      secpOwnerAuthPda = result.authorityPda;
      await sendTx(ctx, result.instructions);

      // Owner adds Secp256r1 spender
      secpSpenderKey = await generateMockSecp256r1Key();
      const ownerSigner = createMockSigner(secpOwnerKey);
      const addResult = await client.addAuthority({
        payer: ctx.payer.publicKey,
        walletPda: secpWalletPda,
        adminSigner: secp256r1(ownerSigner, { authorityPda: secpOwnerAuthPda }),
        newAuthority: {
          type: 'secp256r1',
          credentialIdHash: secpSpenderKey.credentialIdHash,
          compressedPubkey: secpSpenderKey.publicKeyBytes,
          rpId: secpSpenderKey.rpId,
        },
        role: ROLE_SPENDER,
      });
      secpSpenderAuthPda = addResult.newAuthorityPda;
      await sendTx(ctx, addResult.instructions);
    });

    it('secp256r1 spender cannot add authority', async () => {
      const newKp = Keypair.generate();
      const spenderSigner = createMockSigner(secpSpenderKey);

      const { instructions } = await client.addAuthority({
        payer: ctx.payer.publicKey,
        walletPda: secpWalletPda,
        adminSigner: secp256r1(spenderSigner, { authorityPda: secpSpenderAuthPda }),
        newAuthority: { type: 'ed25519', publicKey: newKp.publicKey },
        role: ROLE_SPENDER,
      });

      await sendTxExpectError(ctx, instructions, [], 3002);
    });

    it('secp256r1 spender cannot create session', async () => {
      const sessionKp = Keypair.generate();
      const currentSlot = await getSlot(ctx);
      const spenderSigner = createMockSigner(secpSpenderKey);

      const { instructions } = await client.createSession({
        payer: ctx.payer.publicKey,
        walletPda: secpWalletPda,
        adminSigner: secp256r1(spenderSigner, { authorityPda: secpSpenderAuthPda }),
        sessionKey: sessionKp.publicKey,
        expiresAt: currentSlot + 9000n,
      });

      await sendTxExpectError(ctx, instructions, [], 3002);
    });
  });
});
