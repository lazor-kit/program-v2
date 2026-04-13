import { describe, it, expect, beforeAll } from 'vitest';
import { Keypair, PublicKey } from '@solana/web3.js';
import * as crypto from 'crypto';
import { setupTest, sendTx, type TestContext } from './common';
import { generateMockSecp256r1Key } from './secp256r1Utils';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  createCreateWalletIx,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  PROGRAM_ID,
} from '../../sdk/solita-client/src';
import { AuthorityAccount } from '../../sdk/solita-client/src/generated/accounts';

describe('CreateWallet', () => {
  let ctx: TestContext;

  beforeAll(async () => {
    ctx = await setupTest();
  });

  it('creates a wallet with Ed25519 owner', async () => {
    const ownerKp = Keypair.generate();
    const userSeed = crypto.randomBytes(32);
    const pubkeyBytes = ownerKp.publicKey.toBytes();

    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);
    const [authorityPda, authBump] = findAuthorityPda(walletPda, pubkeyBytes);

    const ix = createCreateWalletIx({
      payer: ctx.payer.publicKey,
      walletPda,
      vaultPda,
      authorityPda,
      userSeed,
      authType: AUTH_TYPE_ED25519,
      authBump,
      credentialOrPubkey: pubkeyBytes,
    });

    await sendTx(ctx, [ix]);

    // Verify wallet account exists
    const walletInfo = await ctx.connection.getAccountInfo(walletPda);
    expect(walletInfo).not.toBeNull();
    expect(walletInfo!.owner.equals(PROGRAM_ID)).toBe(true);

    // Verify authority account
    const authority = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      authorityPda,
    );
    expect(authority.authorityType).toBe(AUTH_TYPE_ED25519);
    expect(authority.role).toBe(0); // Owner
    expect(Number(authority.counter)).toBe(0);
    expect(authority.wallet.equals(walletPda)).toBe(true);
  });

  it('creates a wallet with Secp256r1 owner', async () => {
    const key = await generateMockSecp256r1Key();
    const userSeed = crypto.randomBytes(32);

    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);
    const [authorityPda, authBump] = findAuthorityPda(walletPda, key.credentialIdHash);

    const ix = createCreateWalletIx({
      payer: ctx.payer.publicKey,
      walletPda,
      vaultPda,
      authorityPda,
      userSeed,
      authType: AUTH_TYPE_SECP256R1,
      authBump,
      credentialOrPubkey: key.credentialIdHash,
      secp256r1Pubkey: key.publicKeyBytes,
      rpId: key.rpId,
    });

    await sendTx(ctx, [ix]);

    // Verify authority account
    const authority = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      authorityPda,
    );
    expect(authority.authorityType).toBe(AUTH_TYPE_SECP256R1);
    expect(authority.role).toBe(0); // Owner
    expect(Number(authority.counter)).toBe(0);
  });

  it('rejects duplicate wallet creation', async () => {
    const ownerKp = Keypair.generate();
    const userSeed = crypto.randomBytes(32);
    const pubkeyBytes = ownerKp.publicKey.toBytes();

    const [walletPda] = findWalletPda(userSeed);
    const [vaultPda] = findVaultPda(walletPda);
    const [authorityPda, authBump] = findAuthorityPda(walletPda, pubkeyBytes);

    const ix = createCreateWalletIx({
      payer: ctx.payer.publicKey,
      walletPda,
      vaultPda,
      authorityPda,
      userSeed,
      authType: AUTH_TYPE_ED25519,
      authBump,
      credentialOrPubkey: pubkeyBytes,
    });

    // First creation succeeds
    await sendTx(ctx, [ix]);

    // Second creation should fail
    try {
      await sendTx(ctx, [ix]);
      expect.unreachable('Should have failed');
    } catch (err: any) {
      expect(String(err)).toMatch(/already in use|0x0|uninitialized account/);
    }
  });
});
