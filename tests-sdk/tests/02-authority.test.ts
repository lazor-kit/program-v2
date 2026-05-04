import { describe, it, expect, beforeAll } from 'vitest';
import { Keypair } from '@solana/web3.js';
import * as crypto from 'crypto';
import {
  setupTest,
  sendTx,
  sendTxExpectError,
  PROGRAM_ID,
  type TestContext,
} from './common';
import { generateMockSecp256r1Key, createMockSigner } from './secp256r1Utils';
import {
  LazorKitClient,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  ROLE_ADMIN,
  ROLE_SPENDER,
  ed25519,
  secp256r1,
} from '@lazorkit/sdk-legacy';
import { AuthorityAccount } from '@lazorkit/sdk-legacy';

describe('Authority Management', () => {
  let ctx: TestContext;
  let client: LazorKitClient;

  beforeAll(async () => {
    ctx = await setupTest();
    client = new LazorKitClient(ctx.connection, PROGRAM_ID);
  });

  describe('Ed25519 admin flow', () => {
    let walletPda: any;
    let ownerKp: Keypair;
    let ownerAuthorityPda: any;

    beforeAll(async () => {
      ownerKp = Keypair.generate();
      const userSeed = crypto.randomBytes(32);

      const result = await client.createWallet({
        payer: ctx.payer.publicKey,
        userSeed,
        owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
      });
      walletPda = result.walletPda;
      ownerAuthorityPda = result.authorityPda;

      await sendTx(ctx, result.instructions);
    });

    it('adds an Ed25519 admin authority', async () => {
      const adminKp = Keypair.generate();

      const { instructions, newAuthorityPda } = await client.addAuthority({
        payer: ctx.payer.publicKey,
        walletPda,
        adminSigner: ed25519(ownerKp.publicKey, ownerAuthorityPda),
        newAuthority: { type: 'ed25519', publicKey: adminKp.publicKey },
        role: ROLE_ADMIN,
      });

      await sendTx(ctx, instructions, [ownerKp]);

      const authority = await AuthorityAccount.fromAccountAddress(
        ctx.connection,
        newAuthorityPda,
      );
      expect(authority.authorityType).toBe(AUTH_TYPE_ED25519);
      expect(authority.role).toBe(ROLE_ADMIN);
      expect(Number(authority.counter)).toBe(0);
    });

    it('adds a Secp256r1 spender authority', async () => {
      const key = await generateMockSecp256r1Key();

      const { instructions, newAuthorityPda } = await client.addAuthority({
        payer: ctx.payer.publicKey,
        walletPda,
        adminSigner: ed25519(ownerKp.publicKey, ownerAuthorityPda),
        newAuthority: {
          type: 'secp256r1',
          credentialIdHash: key.credentialIdHash,
          compressedPubkey: key.publicKeyBytes,
          rpId: key.rpId,
        },
        role: ROLE_SPENDER,
      });

      await sendTx(ctx, instructions, [ownerKp]);

      const authority = await AuthorityAccount.fromAccountAddress(
        ctx.connection,
        newAuthorityPda,
      );
      expect(authority.authorityType).toBe(AUTH_TYPE_SECP256R1);
      expect(authority.role).toBe(ROLE_SPENDER);
    });

    it('removes an authority via Ed25519 admin', async () => {
      // First add an authority to remove
      const spenderKp = Keypair.generate();

      const { instructions: addIxs, newAuthorityPda: spenderAuthPda } =
        await client.addAuthority({
          payer: ctx.payer.publicKey,
          walletPda,
          adminSigner: ed25519(ownerKp.publicKey, ownerAuthorityPda),
          newAuthority: { type: 'ed25519', publicKey: spenderKp.publicKey },
          role: ROLE_SPENDER,
        });

      await sendTx(ctx, addIxs, [ownerKp]);

      // Now remove it
      const { instructions: removeIxs } = await client.removeAuthority({
        payer: ctx.payer.publicKey,
        walletPda,
        adminSigner: ed25519(ownerKp.publicKey, ownerAuthorityPda),
        targetAuthorityPda: spenderAuthPda,
      });

      await sendTx(ctx, removeIxs, [ownerKp]);

      // Verify account is closed
      const info = await ctx.connection.getAccountInfo(spenderAuthPda);
      expect(info).toBeNull();
    });

    it('rejects add from non-admin signer', async () => {
      const randomKp = Keypair.generate();
      const newKp = Keypair.generate();

      const { instructions } = await client.addAuthority({
        payer: ctx.payer.publicKey,
        walletPda,
        adminSigner: ed25519(randomKp.publicKey, ownerAuthorityPda),
        newAuthority: { type: 'ed25519', publicKey: newKp.publicKey },
        role: ROLE_SPENDER,
      });

      // Use the random keypair as signer instead of owner — should fail
      await sendTxExpectError(ctx, instructions, [randomKp]);
    });
  });

  describe('Secp256r1 admin flow', () => {
    let walletPda: any;
    let ownerKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
    let ownerAuthorityPda: any;

    beforeAll(async () => {
      ownerKey = await generateMockSecp256r1Key();
      const userSeed = crypto.randomBytes(32);

      const result = await client.createWallet({
        payer: ctx.payer.publicKey,
        userSeed,
        owner: {
          type: 'secp256r1',
          credentialIdHash: ownerKey.credentialIdHash,
          compressedPubkey: ownerKey.publicKeyBytes,
          rpId: ownerKey.rpId,
        },
      });
      walletPda = result.walletPda;
      ownerAuthorityPda = result.authorityPda;

      await sendTx(ctx, result.instructions);
    });

    it('adds an Ed25519 admin via Secp256r1 owner', async () => {
      const adminKp = Keypair.generate();
      const signer = createMockSigner(ownerKey);

      const { instructions, newAuthorityPda } = await client.addAuthority({
        payer: ctx.payer.publicKey,
        walletPda,
        adminSigner: secp256r1(signer, { authorityPda: ownerAuthorityPda }),
        newAuthority: { type: 'ed25519', publicKey: adminKp.publicKey },
        role: ROLE_ADMIN,
      });

      await sendTx(ctx, instructions);

      const authority = await AuthorityAccount.fromAccountAddress(
        ctx.connection,
        newAuthorityPda,
      );
      expect(authority.authorityType).toBe(AUTH_TYPE_ED25519);
      expect(authority.role).toBe(ROLE_ADMIN);
    });
  });
});
