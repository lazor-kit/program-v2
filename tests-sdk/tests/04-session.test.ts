import { describe, it, expect, beforeAll } from 'vitest';
import { Keypair } from '@solana/web3.js';
import * as crypto from 'crypto';
import {
  setupTest,
  sendTx,
  sendTxExpectError,
  getSlot,
  type TestContext,
} from './common';
import { LazorKitClient, AUTH_TYPE_ED25519 } from '../../sdk/solita-client/src';
import { SessionAccount } from '../../sdk/solita-client/src/generated/accounts';

describe('CreateSession', () => {
  let ctx: TestContext;
  let client: LazorKitClient;
  let walletPda: any;
  let ownerKp: Keypair;
  let ownerAuthorityPda: any;

  beforeAll(async () => {
    ctx = await setupTest();
    client = new LazorKitClient(ctx.connection);

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

  it('creates a session with Ed25519 admin', async () => {
    const sessionKp = Keypair.generate();
    const sessionKeyBytes = sessionKp.publicKey.toBytes();

    // Expires ~1 hour from now in slots (~2.5 slots/sec * 3600 = 9000 slots)
    const currentSlot = await getSlot(ctx);
    const expiresAt = currentSlot + 9000n;

    const { ix, sessionPda } = client.createSessionEd25519({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: ownerAuthorityPda,
      adminSigner: ownerKp.publicKey,
      sessionKey: sessionKeyBytes,
      expiresAt,
    });

    await sendTx(ctx, [ix], [ownerKp]);

    // Verify session
    const session = await SessionAccount.fromAccountAddress(
      ctx.connection,
      sessionPda,
    );
    expect(session.wallet.equals(walletPda)).toBe(true);
    expect(session.sessionKey.equals(sessionKp.publicKey)).toBe(true);
    expect(Number(session.expiresAt)).toBe(Number(expiresAt));
  });

  it('rejects session creation from unauthorized signer', async () => {
    const randomKp = Keypair.generate();
    const sessionKp = Keypair.generate();

    const expiresAt = BigInt(Math.floor(Date.now() / 1000) + 3600);

    const { ix } = client.createSessionEd25519({
      payer: ctx.payer.publicKey,
      walletPda,
      adminAuthorityPda: ownerAuthorityPda,
      adminSigner: randomKp.publicKey,
      sessionKey: sessionKp.publicKey.toBytes(),
      expiresAt,
    });

    await sendTxExpectError(ctx, [ix], [randomKp]);
  });
});
