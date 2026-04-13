import { describe, it, expect, beforeAll } from 'vitest';
import { Keypair } from '@solana/web3.js';
import * as crypto from 'crypto';
import {
  setupTest,
  sendTx,
  sendTxExpectError,
  type TestContext,
} from './common';
import { generateMockSecp256r1Key, createMockSigner } from './secp256r1Utils';
import {
  LazorKitClient,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  ROLE_ADMIN,
  ROLE_SPENDER,
} from '../../sdk/solita-client/src';
import { AuthorityAccount } from '../../sdk/solita-client/src/generated/accounts';

describe('Authority Management', () => {
  let ctx: TestContext;
  let client: LazorKitClient;

  beforeAll(async () => {
    ctx = await setupTest();
    client = new LazorKitClient(ctx.connection);
  });

  describe('Ed25519 admin flow', () => {
    let walletPda: any;
    let ownerKp: Keypair;
    let ownerAuthorityPda: any;

    beforeAll(async () => {
      ownerKp = Keypair.generate();
      const userSeed = crypto.randomBytes(32);

      const result = client.createWalletEd25519({
        payer: ctx.payer.publicKey,
        userSeed,
        ownerPubkey: ownerKp.publicKey,
      });
      walletPda = result.walletPda;
      ownerAuthorityPda = result.authorityPda;

      await sendTx(ctx, [result.ix]);
    });

    it('adds an Ed25519 admin authority', async () => {
      const adminKp = Keypair.generate();

      const { ix, newAuthorityPda } = client.addAuthorityEd25519({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        adminSigner: ownerKp.publicKey,
        newType: AUTH_TYPE_ED25519,
        newRole: ROLE_ADMIN,
        newCredentialOrPubkey: adminKp.publicKey.toBytes(),
      });

      await sendTx(ctx, [ix], [ownerKp]);

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

      const { ix, newAuthorityPda } = client.addAuthorityEd25519({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        adminSigner: ownerKp.publicKey,
        newType: AUTH_TYPE_SECP256R1,
        newRole: ROLE_SPENDER,
        newCredentialOrPubkey: key.credentialIdHash,
        newSecp256r1Pubkey: key.publicKeyBytes,
        newRpId: key.rpId,
      });

      await sendTx(ctx, [ix], [ownerKp]);

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

      const { ix: addIx, newAuthorityPda: spenderAuthPda } =
        client.addAuthorityEd25519({
          payer: ctx.payer.publicKey,
          walletPda,
          adminAuthorityPda: ownerAuthorityPda,
          adminSigner: ownerKp.publicKey,
          newType: AUTH_TYPE_ED25519,
          newRole: ROLE_SPENDER,
          newCredentialOrPubkey: spenderKp.publicKey.toBytes(),
        });

      await sendTx(ctx, [addIx], [ownerKp]);

      // Now remove it
      const removeIx = client.removeAuthorityEd25519({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        adminSigner: ownerKp.publicKey,
        targetAuthorityPda: spenderAuthPda,
      });

      await sendTx(ctx, [removeIx], [ownerKp]);

      // Verify account is closed
      const info = await ctx.connection.getAccountInfo(spenderAuthPda);
      expect(info).toBeNull();
    });

    it('rejects add from non-admin signer', async () => {
      const randomKp = Keypair.generate();
      const newKp = Keypair.generate();

      const { ix } = client.addAuthorityEd25519({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        adminSigner: randomKp.publicKey,
        newType: AUTH_TYPE_ED25519,
        newRole: ROLE_SPENDER,
        newCredentialOrPubkey: newKp.publicKey.toBytes(),
      });

      // Use the random keypair as signer instead of owner — should fail
      await sendTxExpectError(ctx, [ix], [randomKp]);
    });
  });

  describe('Secp256r1 admin flow', () => {
    let walletPda: any;
    let ownerKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
    let ownerAuthorityPda: any;

    beforeAll(async () => {
      ownerKey = await generateMockSecp256r1Key();
      const userSeed = crypto.randomBytes(32);

      const result = client.createWalletSecp256r1({
        payer: ctx.payer.publicKey,
        userSeed,
        credentialIdHash: ownerKey.credentialIdHash,
        compressedPubkey: ownerKey.publicKeyBytes,
        rpId: ownerKey.rpId,
      });
      walletPda = result.walletPda;
      ownerAuthorityPda = result.authorityPda;

      await sendTx(ctx, [result.ix]);
    });

    it('adds an Ed25519 admin via Secp256r1 owner', async () => {
      const adminKp = Keypair.generate();
      const signer = createMockSigner(ownerKey);

      const { ix, newAuthorityPda, precompileIx } =
        await client.addAuthoritySecp256r1({
          payer: ctx.payer.publicKey,
          walletPda,
          adminAuthorityPda: ownerAuthorityPda,
          adminSigner: signer,
          newType: AUTH_TYPE_ED25519,
          newRole: ROLE_ADMIN,
          newCredentialOrPubkey: adminKp.publicKey.toBytes(),
        });

      await sendTx(ctx, [precompileIx, ix]);

      const authority = await AuthorityAccount.fromAccountAddress(
        ctx.connection,
        newAuthorityPda,
      );
      expect(authority.authorityType).toBe(AUTH_TYPE_ED25519);
      expect(authority.role).toBe(ROLE_ADMIN);
    });
  });
});
