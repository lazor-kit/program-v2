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
import { LazorKitClient, ed25519 } from '../../sdk/solita-client/src';
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

    const result = client.createWallet({
      payer: ctx.payer.publicKey,
      userSeed,
      owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
    });
    walletPda = result.walletPda;
    ownerAuthorityPda = result.authorityPda;

    await sendTx(ctx, result.instructions);
  });

  it('creates a session with Ed25519 admin', async () => {
    const sessionKp = Keypair.generate();

    // Expires ~1 hour from now in slots (~2.5 slots/sec * 3600 = 9000 slots)
    const currentSlot = await getSlot(ctx);
    const expiresAt = currentSlot + 9000n;

    const { instructions, sessionPda } = await client.createSession({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthorityPda),
      sessionKey: sessionKp.publicKey,
      expiresAt,
    });

    await sendTx(ctx, instructions, [ownerKp]);

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

    const { instructions } = await client.createSession({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(randomKp.publicKey, ownerAuthorityPda),
      sessionKey: sessionKp.publicKey,
      expiresAt,
    });

    await sendTxExpectError(ctx, instructions, [randomKp]);
  });
});
