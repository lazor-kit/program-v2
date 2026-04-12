import { describe, it, expect, beforeAll } from 'vitest';
import { Keypair } from '@solana/web3.js';
import * as crypto from 'crypto';
import { setupTest, sendTx, sendTxExpectError, getSlot, type TestContext } from './common';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  createCreateWalletIx,
  createCreateSessionIx,
  AUTH_TYPE_ED25519,
} from '../../sdk/solita-client/src';
import { SessionAccount } from '../../sdk/solita-client/src/generated/accounts';

describe('CreateSession', () => {
  let ctx: TestContext;
  let walletPda: any;
  let ownerKp: Keypair;
  let ownerAuthorityPda: any;

  beforeAll(async () => {
    ctx = await setupTest();

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

  it('creates a session with Ed25519 admin', async () => {
    const sessionKp = Keypair.generate();
    const sessionKeyBytes = sessionKp.publicKey.toBytes();
    const [sessionPda] = findSessionPda(walletPda, sessionKeyBytes);

    // Expires ~1 hour from now in slots (~2.5 slots/sec * 3600 = 9000 slots)
    const currentSlot = await getSlot(ctx);
    const expiresAt = currentSlot + 9000n;

    const ix = createCreateSessionIx({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: ownerAuthorityPda,
      sessionPda,
      sessionKey: sessionKeyBytes,
      expiresAt,
      authorizerSigner: ownerKp.publicKey,
    });

    await sendTx(ctx, [ix], [ownerKp]);

    // Verify session
    const session = await SessionAccount.fromAccountAddress(ctx.connection, sessionPda);
    expect(session.wallet.equals(walletPda)).toBe(true);
    expect(session.sessionKey.equals(sessionKp.publicKey)).toBe(true);
    expect(Number(session.expiresAt)).toBe(Number(expiresAt));
  });

  it('rejects session creation from unauthorized signer', async () => {
    const randomKp = Keypair.generate();
    const sessionKp = Keypair.generate();
    const [sessionPda] = findSessionPda(walletPda, sessionKp.publicKey.toBytes());

    const expiresAt = BigInt(Math.floor(Date.now() / 1000) + 3600);

    const ix = createCreateSessionIx({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: ownerAuthorityPda,
      sessionPda,
      sessionKey: sessionKp.publicKey.toBytes(),
      expiresAt,
      authorizerSigner: randomKp.publicKey,
    });

    await sendTxExpectError(ctx, [ix], [randomKp]);
  });
});
