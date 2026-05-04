/**
 * Session-based execute tests — the critical path that was completely untested.
 *
 * Tests:
 * - Creating a session and executing via session key
 * - Session expiry enforcement
 * - Wrong session key rejection
 */
import { describe, it, expect, beforeAll } from 'vitest';
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import * as crypto from 'crypto';
import {
  setupTest,
  sendTx,
  sendTxExpectError,
  getSlot,
  type TestContext,
} from './common';
import {
  LazorKitClient,
  ed25519,
  session,
} from '@lazorkit/sdk-legacy';

describe('Session Execute', () => {
  let ctx: TestContext;
  let client: LazorKitClient;

  let walletPda: PublicKey;
  let vaultPda: PublicKey;
  let ownerKp: Keypair;
  let ownerAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();
    client = new LazorKitClient(ctx.connection);

    ownerKp = Keypair.generate();
    const userSeed = crypto.randomBytes(32);

    const result = await client.createWallet({
      payer: ctx.payer.publicKey,
      userSeed,
      owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
    });
    walletPda = result.walletPda;
    vaultPda = result.vaultPda;
    ownerAuthPda = result.authorityPda;

    await sendTx(ctx, result.instructions);

    // Fund the vault
    const sig = await ctx.connection.requestAirdrop(vaultPda, 5 * LAMPORTS_PER_SOL);
    await ctx.connection.confirmTransaction(sig, 'confirmed');
  });

  it('executes SOL transfer via session key', async () => {
    const sessionKp = Keypair.generate();
    const currentSlot = await getSlot(ctx);
    const expiresAt = currentSlot + 9000n; // ~1 hour

    // Create session
    const { instructions: createIxs, sessionPda } = await client.createSession({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      sessionKey: sessionKp.publicKey,
      expiresAt,
    });
    await sendTx(ctx, createIxs, [ownerKp]);

    // Execute via session
    const recipient = Keypair.generate().publicKey;
    const { instructions: execIxs } = await client.execute({
      payer: ctx.payer.publicKey,
      walletPda,
      signer: session(sessionPda, sessionKp.publicKey),
      instructions: [
        SystemProgram.transfer({
          fromPubkey: vaultPda,
          toPubkey: recipient,
          lamports: 1_000_000,
        }),
      ],
    });

    const balanceBefore = await ctx.connection.getBalance(recipient);
    await sendTx(ctx, execIxs, [sessionKp]);
    const balanceAfter = await ctx.connection.getBalance(recipient);

    expect(balanceAfter - balanceBefore).toBe(1_000_000);
  });

  it('transferSol works via session key', async () => {
    const sessionKp = Keypair.generate();
    const currentSlot = await getSlot(ctx);

    const { instructions: createIxs, sessionPda } = await client.createSession({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      sessionKey: sessionKp.publicKey,
      expiresAt: currentSlot + 9000n,
    });
    await sendTx(ctx, createIxs, [ownerKp]);

    const recipient = Keypair.generate().publicKey;
    const { instructions } = await client.transferSol({
      payer: ctx.payer.publicKey,
      walletPda,
      signer: session(sessionPda, sessionKp.publicKey),
      recipient,
      lamports: 1_000_000,
    });

    const balanceBefore = await ctx.connection.getBalance(recipient);
    await sendTx(ctx, instructions, [sessionKp]);
    const balanceAfter = await ctx.connection.getBalance(recipient);

    expect(balanceAfter - balanceBefore).toBe(1_000_000);
  });

  it('rejects execution with wrong session key', async () => {
    const sessionKp = Keypair.generate();
    const wrongKp = Keypair.generate();
    const currentSlot = await getSlot(ctx);

    const { instructions: createIxs, sessionPda } = await client.createSession({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      sessionKey: sessionKp.publicKey,
      expiresAt: currentSlot + 9000n,
    });
    await sendTx(ctx, createIxs, [ownerKp]);

    // Try to execute with wrong key but same session PDA
    const recipient = Keypair.generate().publicKey;
    const { instructions } = await client.execute({
      payer: ctx.payer.publicKey,
      walletPda,
      signer: session(sessionPda, wrongKp.publicKey),
      instructions: [
        SystemProgram.transfer({
          fromPubkey: vaultPda,
          toPubkey: recipient,
          lamports: 1_000_000,
        }),
      ],
    });

    // The wrong key doesn't match session_key stored on-chain
    await sendTxExpectError(ctx, instructions, [wrongKp]);
  });

  it('rejects execution with expired session', async () => {
    const sessionKp = Keypair.generate();
    const currentSlot = await getSlot(ctx);

    // Create a session that expires in ~10 slots (very short)
    const expiresAt = currentSlot + 10n;

    const { instructions: createIxs, sessionPda } = await client.createSession({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      sessionKey: sessionKp.publicKey,
      expiresAt,
    });
    await sendTx(ctx, createIxs, [ownerKp]);

    // Wait for the session to expire (~4 seconds at ~2.5 slots/sec)
    await new Promise(resolve => setTimeout(resolve, 5000));

    const recipient = Keypair.generate().publicKey;
    const { instructions } = await client.execute({
      payer: ctx.payer.publicKey,
      walletPda,
      signer: session(sessionPda, sessionKp.publicKey),
      instructions: [
        SystemProgram.transfer({
          fromPubkey: vaultPda,
          toPubkey: recipient,
          lamports: 1_000_000,
        }),
      ],
    });

    // Error 3009 = SessionExpired
    await sendTxExpectError(ctx, instructions, [sessionKp], 3009);
  });
});
