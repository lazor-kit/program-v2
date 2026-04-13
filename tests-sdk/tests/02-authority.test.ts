import { describe, it, expect, beforeAll } from 'vitest';
import { Keypair } from '@solana/web3.js';
import * as crypto from 'crypto';
import { setupTest, sendTx, sendTxExpectError, getSlot, type TestContext } from './common';
import { generateMockSecp256r1Key, signSecp256r1 } from './secp256r1Utils';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  createCreateWalletIx,
  createAddAuthorityIx,
  createRemoveAuthorityIx,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  ROLE_ADMIN,
  ROLE_SPENDER,
  DISC_ADD_AUTHORITY,
  DISC_REMOVE_AUTHORITY,
  PROGRAM_ID,
} from '../../sdk/solita-client/src';
import { AuthorityAccount } from '../../sdk/solita-client/src/generated/accounts';

describe('Authority Management', () => {
  let ctx: TestContext;

  beforeAll(async () => {
    ctx = await setupTest();
  });

  describe('Ed25519 admin flow', () => {
    let walletPda: any;
    let ownerKp: Keypair;
    let ownerAuthorityPda: any;

    beforeAll(async () => {
      ownerKp = Keypair.generate();
      const userSeed = crypto.randomBytes(32);
      const pubkeyBytes = ownerKp.publicKey.toBytes();

      [walletPda] = findWalletPda(userSeed);
      const [vaultPda] = findVaultPda(walletPda);
      const [authPda, authBump] = findAuthorityPda(walletPda, pubkeyBytes);
      ownerAuthorityPda = authPda;

      await sendTx(ctx, [createCreateWalletIx({
        payer: ctx.payer.publicKey,
        walletPda,
        vaultPda,
        authorityPda: authPda,
        userSeed,
        authType: AUTH_TYPE_ED25519,
        authBump,
        credentialOrPubkey: pubkeyBytes,
      })]);
    });

    it('adds an Ed25519 admin authority', async () => {
      const adminKp = Keypair.generate();
      const adminPubkey = adminKp.publicKey.toBytes();
      const [newAuthPda] = findAuthorityPda(walletPda, adminPubkey);

      const ix = createAddAuthorityIx({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        newAuthorityPda: newAuthPda,
        newType: AUTH_TYPE_ED25519,
        newRole: ROLE_ADMIN,
        credentialOrPubkey: adminPubkey,
        authorizerSigner: ownerKp.publicKey,
      });

      await sendTx(ctx, [ix], [ownerKp]);

      const authority = await AuthorityAccount.fromAccountAddress(ctx.connection, newAuthPda);
      expect(authority.authorityType).toBe(AUTH_TYPE_ED25519);
      expect(authority.role).toBe(ROLE_ADMIN);
      expect(Number(authority.counter)).toBe(0);
    });

    it('adds a Secp256r1 spender authority', async () => {
      const key = await generateMockSecp256r1Key();
      const [newAuthPda] = findAuthorityPda(walletPda, key.credentialIdHash);

      const ix = createAddAuthorityIx({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        newAuthorityPda: newAuthPda,
        newType: AUTH_TYPE_SECP256R1,
        newRole: ROLE_SPENDER,
        credentialOrPubkey: key.credentialIdHash,
        secp256r1Pubkey: key.publicKeyBytes,
        rpId: key.rpId,
        authorizerSigner: ownerKp.publicKey,
      });

      await sendTx(ctx, [ix], [ownerKp]);

      const authority = await AuthorityAccount.fromAccountAddress(ctx.connection, newAuthPda);
      expect(authority.authorityType).toBe(AUTH_TYPE_SECP256R1);
      expect(authority.role).toBe(ROLE_SPENDER);
    });

    it('removes an authority via Ed25519 admin', async () => {
      // First add an authority to remove
      const spenderKp = Keypair.generate();
      const spenderPubkey = spenderKp.publicKey.toBytes();
      const [spenderAuthPda] = findAuthorityPda(walletPda, spenderPubkey);

      await sendTx(ctx, [createAddAuthorityIx({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        newAuthorityPda: spenderAuthPda,
        newType: AUTH_TYPE_ED25519,
        newRole: ROLE_SPENDER,
        credentialOrPubkey: spenderPubkey,
        authorizerSigner: ownerKp.publicKey,
      })], [ownerKp]);

      // Now remove it
      const removeIx = createRemoveAuthorityIx({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        targetAuthorityPda: spenderAuthPda,
        refundDestination: ctx.payer.publicKey,
        authorizerSigner: ownerKp.publicKey,
      });

      await sendTx(ctx, [removeIx], [ownerKp]);

      // Verify account is closed
      const info = await ctx.connection.getAccountInfo(spenderAuthPda);
      expect(info).toBeNull();
    });

    it('rejects add from non-admin signer', async () => {
      const randomKp = Keypair.generate();
      const newKp = Keypair.generate();
      const [newAuthPda] = findAuthorityPda(walletPda, newKp.publicKey.toBytes());

      const ix = createAddAuthorityIx({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        newAuthorityPda: newAuthPda,
        newType: AUTH_TYPE_ED25519,
        newRole: ROLE_SPENDER,
        credentialOrPubkey: newKp.publicKey.toBytes(),
        authorizerSigner: randomKp.publicKey,
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

      [walletPda] = findWalletPda(userSeed);
      const [vaultPda] = findVaultPda(walletPda);
      const [authPda, authBump] = findAuthorityPda(walletPda, ownerKey.credentialIdHash);
      ownerAuthorityPda = authPda;

      await sendTx(ctx, [createCreateWalletIx({
        payer: ctx.payer.publicKey,
        walletPda,
        vaultPda,
        authorityPda: authPda,
        userSeed,
        authType: AUTH_TYPE_SECP256R1,
        authBump,
        credentialOrPubkey: ownerKey.credentialIdHash,
        secp256r1Pubkey: ownerKey.publicKeyBytes,
        rpId: ownerKey.rpId,
      })]);
    });

    it('adds an Ed25519 admin via Secp256r1 owner', async () => {
      const adminKp = Keypair.generate();
      const adminPubkey = adminKp.publicKey.toBytes();
      const [newAuthPda] = findAuthorityPda(walletPda, adminPubkey);
      const slot = await getSlot(ctx);

      // Build the data payload (matches on-chain split logic)
      // On-chain: data_payload = instruction_data[0..8+full_auth_data.len()]
      // For Ed25519 target: data_payload = args(8) + pubkey(32) = 40 bytes
      const dataPayload = Buffer.concat([
        Buffer.from([AUTH_TYPE_ED25519, ROLE_ADMIN]),
        Buffer.alloc(6), // padding
        adminPubkey,
      ]);

      // On-chain extends: extended_data_payload = data_payload + payer.key()
      const signedPayload = Buffer.concat([dataPayload, ctx.payer.publicKey.toBuffer()]);

      // sysvar_instructions is at index 6 (after rent at 5)
      const { authPayload, precompileIx } = await signSecp256r1({
        key: ownerKey,
        discriminator: new Uint8Array([DISC_ADD_AUTHORITY]),
        signedPayload,
        slot,
        counter: 1, // first use, stored counter = 0
        payer: ctx.payer.publicKey,
        sysvarIxIndex: 6,
      });

      const ix = createAddAuthorityIx({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthorityPda,
        newAuthorityPda: newAuthPda,
        newType: AUTH_TYPE_ED25519,
        newRole: ROLE_ADMIN,
        credentialOrPubkey: adminPubkey,
        authPayload,
      });

      await sendTx(ctx, [precompileIx, ix]);

      const authority = await AuthorityAccount.fromAccountAddress(ctx.connection, newAuthPda);
      expect(authority.authorityType).toBe(AUTH_TYPE_ED25519);
      expect(authority.role).toBe(ROLE_ADMIN);
    });
  });
});
