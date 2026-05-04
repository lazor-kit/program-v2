import { describe, it, expect, beforeAll } from 'vitest';
import { Keypair } from '@solana/web3.js';
import * as crypto from 'crypto';
import { setupTest, sendTx, PROGRAM_ID, type TestContext } from './common';
import { generateMockSecp256r1Key } from './secp256r1Utils';
import {
  LazorKitClient,
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
  AuthorityAccount,
} from '@lazorkit/sdk-legacy';

describe('CreateWallet', () => {
  let ctx: TestContext;
  let client: LazorKitClient;

  beforeAll(async () => {
    ctx = await setupTest();
    client = new LazorKitClient(ctx.connection, PROGRAM_ID);
  });

  it('creates a wallet with Ed25519 owner', async () => {
    const ownerKp = Keypair.generate();
    const userSeed = crypto.randomBytes(32);

    const { instructions, walletPda, authorityPda } = await client.createWallet({
      payer: ctx.payer.publicKey,
      userSeed,
      owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
    });

    await sendTx(ctx, instructions);

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

    const { instructions, authorityPda } = await client.createWallet({
      payer: ctx.payer.publicKey,
      userSeed,
      owner: {
        type: 'secp256r1',
        credentialIdHash: key.credentialIdHash,
        compressedPubkey: key.publicKeyBytes,
        rpId: key.rpId,
      },
    });

    await sendTx(ctx, instructions);

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

    const { instructions } = await client.createWallet({
      payer: ctx.payer.publicKey,
      userSeed,
      owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
    });

    // First creation succeeds
    await sendTx(ctx, instructions);

    // Second creation should fail
    try {
      await sendTx(ctx, instructions);
      expect.unreachable('Should have failed');
    } catch (err: any) {
      expect(String(err)).toMatch(/already in use|0x0|uninitialized account/);
    }
  });
});
