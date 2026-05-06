/**
 * Session action enforcement (E2E).
 *
 * P1c added the action enforcement engine on-chain (program-v2 has been
 * exercising it via litesvm Rust tests). This file is the TypeScript-side
 * dogfood: every assertion goes through @lazorkit/sdk-legacy's Actions
 * builder and serialiser, then submits a real tx and watches what the
 * program does to it.
 *
 * Coverage:
 * - ProgramWhitelist: allow whitelisted CPI, reject non-whitelisted (3021)
 * - ProgramBlacklist: reject blacklisted CPI (3022)
 * - SolMaxPerTx: allow ≤ max, reject > max (3023)
 * - SolLimit (lifetime): allow within budget, reject after exhaust (3024)
 * - Bonus: session WITHOUT actions is unrestricted (sanity baseline)
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
  PROGRAM_ID,
  type TestContext,
} from './common';
import { Actions, LazorKitClient, ed25519, session } from '@lazorkit/sdk-legacy';

// MEMO program — innocuous CPI target for whitelist tests
const MEMO_PROGRAM_ID = new PublicKey(
  'MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr',
);

describe('Session Actions (E2E enforcement)', () => {
  let ctx: TestContext;
  let client: LazorKitClient;

  let walletPda: PublicKey;
  let vaultPda: PublicKey;
  let ownerKp: Keypair;
  let ownerAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();
    // Pass PROGRAM_ID explicitly — sdk-legacy's URL-based auto-infer
    // defaults localhost to 4h3X… (commercial devnet), but the validator
    // here loads program-v2's foundation binary at PROGRAM_ID (resolved
    // from target/deploy/lazorkit_program-keypair.json).
    client = new LazorKitClient(ctx.connection, PROGRAM_ID);

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

    // Fund the vault generously so spending caps are the constraint, not balance
    const sig = await ctx.connection.requestAirdrop(
      vaultPda,
      10 * LAMPORTS_PER_SOL,
    );
    await ctx.connection.confirmTransaction(sig, 'confirmed');
  });

  /**
   * Helper: create a session with the given actions, return its (sessionPda, sessionKp).
   */
  async function createSessionWithActions(
    actions: Parameters<typeof client.createSession>[0]['actions'],
  ): Promise<{ sessionPda: PublicKey; sessionKp: Keypair }> {
    const sessionKp = Keypair.generate();
    const currentSlot = await getSlot(ctx);
    const expiresAt = currentSlot + 9000n; // ~1h

    const { instructions, sessionPda } = await client.createSession({
      payer: ctx.payer.publicKey,
      walletPda,
      adminSigner: ed25519(ownerKp.publicKey, ownerAuthPda),
      sessionKey: sessionKp.publicKey,
      expiresAt,
      actions,
    });
    await sendTx(ctx, instructions, [ownerKp]);

    return { sessionPda, sessionKp };
  }

  /**
   * Helper: build an execute tx that transfers `lamports` from vault to the
   * test payer (an existing funded account, so SystemProgram.transfer doesn't
   * trip the rent-exempt minimum check that fresh accounts hit). The action
   * enforcement we're testing kicks in BEFORE the inner transfer runs, so
   * the recipient choice doesn't affect what we're verifying.
   */
  async function buildVaultTransferIxs(
    sessionPda: PublicKey,
    sessionKey: PublicKey,
    lamports: number,
  ): Promise<{ ixs: Awaited<ReturnType<typeof client.execute>>['instructions']; recipient: PublicKey }> {
    const recipient = ctx.payer.publicKey;
    const { instructions } = await client.execute({
      payer: ctx.payer.publicKey,
      walletPda,
      signer: session(sessionPda, sessionKey),
      instructions: [
        SystemProgram.transfer({
          fromPubkey: vaultPda,
          toPubkey: recipient,
          lamports,
        }),
      ],
    });
    return { ixs: instructions, recipient };
  }

  /** Snapshot vault balance, run sendTx, return vault delta (positive = outflow). */
  async function txAndVaultDelta(
    ixs: Awaited<ReturnType<typeof client.execute>>['instructions'],
    sessionKp: Keypair,
  ): Promise<number> {
    const before = await ctx.connection.getBalance(vaultPda);
    await sendTx(ctx, ixs, [sessionKp]);
    const after = await ctx.connection.getBalance(vaultPda);
    return before - after;
  }

  // ─── Baseline ──────────────────────────────────────────────────────

  it('session without actions is unrestricted (baseline)', async () => {
    // No actions arg → unrestricted session, any execute should pass
    const { sessionPda, sessionKp } = await createSessionWithActions(undefined);

    const { ixs } = await buildVaultTransferIxs(
      sessionPda,
      sessionKp.publicKey,
      500_000,
    );
    expect(await txAndVaultDelta(ixs, sessionKp)).toBe(500_000);
  });

  // ─── ProgramWhitelist (action type 10) ─────────────────────────────

  it('ProgramWhitelist: allows CPI to whitelisted program', async () => {
    // Whitelist System Program → SOL transfer (which CPIs to System Program) should succeed
    const { sessionPda, sessionKp } = await createSessionWithActions([
      Actions.programWhitelist(SystemProgram.programId),
    ]);

    const { ixs } = await buildVaultTransferIxs(
      sessionPda,
      sessionKp.publicKey,
      300_000,
    );
    expect(await txAndVaultDelta(ixs, sessionKp)).toBe(300_000);
  });

  it('ProgramWhitelist: rejects CPI to non-whitelisted program (3021)', async () => {
    // Whitelist ONLY Memo program → SOL transfer (System Program CPI) must fail
    const { sessionPda, sessionKp } = await createSessionWithActions([
      Actions.programWhitelist(MEMO_PROGRAM_ID),
    ]);

    const { ixs } = await buildVaultTransferIxs(
      sessionPda,
      sessionKp.publicKey,
      100_000,
    );
    // 3021 = ActionProgramNotWhitelisted
    await sendTxExpectError(ctx, ixs, [sessionKp], 3021);
  });

  // ─── ProgramBlacklist (action type 11) ─────────────────────────────

  it('ProgramBlacklist: rejects CPI to blacklisted program (3022)', async () => {
    // Blacklist System Program → SOL transfer must fail
    const { sessionPda, sessionKp } = await createSessionWithActions([
      Actions.programBlacklist(SystemProgram.programId),
    ]);

    const { ixs } = await buildVaultTransferIxs(
      sessionPda,
      sessionKp.publicKey,
      100_000,
    );
    // 3022 = ActionProgramBlacklisted
    await sendTxExpectError(ctx, ixs, [sessionKp], 3022);
  });

  it('ProgramBlacklist: allows CPI to non-blacklisted program', async () => {
    // Blacklist Memo → SOL transfer (System Program CPI) is fine
    const { sessionPda, sessionKp } = await createSessionWithActions([
      Actions.programBlacklist(MEMO_PROGRAM_ID),
    ]);

    const { ixs } = await buildVaultTransferIxs(
      sessionPda,
      sessionKp.publicKey,
      200_000,
    );
    expect(await txAndVaultDelta(ixs, sessionKp)).toBe(200_000);
  });

  // ─── SolMaxPerTx (action type 3) ───────────────────────────────────

  it('SolMaxPerTx: allows transfer at the cap', async () => {
    const { sessionPda, sessionKp } = await createSessionWithActions([
      Actions.solMaxPerTx(500_000n),
    ]);

    const { ixs } = await buildVaultTransferIxs(
      sessionPda,
      sessionKp.publicKey,
      500_000, // exactly the cap
    );
    expect(await txAndVaultDelta(ixs, sessionKp)).toBe(500_000);
  });

  it('SolMaxPerTx: rejects transfer above the cap (3023)', async () => {
    const { sessionPda, sessionKp } = await createSessionWithActions([
      Actions.solMaxPerTx(500_000n),
    ]);

    const { ixs } = await buildVaultTransferIxs(
      sessionPda,
      sessionKp.publicKey,
      500_001, // 1 lamport over
    );
    // 3023 = ActionSolMaxPerTxExceeded
    await sendTxExpectError(ctx, ixs, [sessionKp], 3023);
  });

  // ─── SolLimit (lifetime cap, action type 1) ────────────────────────

  it('SolLimit: allows spending within lifetime budget then rejects when exhausted (3024)', async () => {
    // Lifetime budget = 1_000_000. First tx spends 700k → OK.
    // Second tx tries 400k → would exceed remaining 300k → reject 3024.
    const { sessionPda, sessionKp } = await createSessionWithActions([
      Actions.solLimit(1_000_000n),
    ]);

    // 1st: spend 700_000 — fits
    {
      const { ixs } = await buildVaultTransferIxs(
        sessionPda,
        sessionKp.publicKey,
        700_000,
      );
      expect(await txAndVaultDelta(ixs, sessionKp)).toBe(700_000);
    }

    // 2nd: spend 400_000 — exceeds remaining 300_000 → 3024
    {
      const { ixs } = await buildVaultTransferIxs(
        sessionPda,
        sessionKp.publicKey,
        400_000,
      );
      // 3024 = ActionSolLimitExceeded
      await sendTxExpectError(ctx, ixs, [sessionKp], 3024);
    }

    // 3rd: spend exactly remaining 300_000 — should succeed
    {
      const { ixs } = await buildVaultTransferIxs(
        sessionPda,
        sessionKp.publicKey,
        300_000,
      );
      expect(await txAndVaultDelta(ixs, sessionKp)).toBe(300_000);
    }
  });

  // ─── Combined actions ──────────────────────────────────────────────

  it('Combined: ProgramWhitelist + SolMaxPerTx both enforced', async () => {
    // Whitelist System Program, cap per-tx at 250_000
    const { sessionPda, sessionKp } = await createSessionWithActions([
      Actions.programWhitelist(SystemProgram.programId),
      Actions.solMaxPerTx(250_000n),
    ]);

    // Within both rules → OK
    {
      const { ixs } = await buildVaultTransferIxs(
        sessionPda,
        sessionKp.publicKey,
        250_000,
      );
      expect(await txAndVaultDelta(ixs, sessionKp)).toBe(250_000);
    }

    // Exceed SolMaxPerTx but System Program allowed → 3023
    {
      const { ixs } = await buildVaultTransferIxs(
        sessionPda,
        sessionKp.publicKey,
        250_001,
      );
      await sendTxExpectError(ctx, ixs, [sessionKp], 3023);
    }
  });
});
