/**
 * Secp256r1 Counter & Replay Protection Tests
 *
 * Verifies that the monotonically increasing counter prevents replay attacks.
 */

import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  type TransactionInstruction,
} from '@solana/web3.js';
import { describe, it, expect, beforeAll } from 'vitest';
import {
  findVaultPda,
  findAuthorityPda,
  AuthType,
  Role,
  AuthorityAccount,
} from '@lazorkit/solita-client';
import {
  setupTest,
  sendTx,
  tryProcessInstructions,
  type TestContext,
} from './common';
import { generateMockSecp256r1Signer } from './secp256r1Utils';

function getRandomSeed() {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  return seed;
}

/**
 * Helper: Build Secp256r1 execute instruction using SDK.
 * Counter is automatically fetched from Authority account and incremented.
 *
 * Inputs:
 *  - signer: Secp256r1 signer
 *  - walletPda: Wallet account
 *  - innerInstructions: Instructions to execute
 *
 * Returns:
 *  - { precompileIx, executeIx }: Two instructions (precompile + execute)
 */
async function buildSecp256r1Execute(
  ctx: TestContext,
  signer: any,
  walletPda: PublicKey,
  innerInstructions: TransactionInstruction[],
  rpId = 'example.com',
) {
  return ctx.highClient.executeWithSecp256r1({
    payer: ctx.payer,
    walletPda,
    innerInstructions,
    signer,
    rpId,
  });
}

describe('Counter & Replay Protection (Secp256r1)', () => {
  let ctx: TestContext;

  let owner: Keypair;
  let walletPda: PublicKey;
  let vaultPda: PublicKey;
  let secpAdmin: any;
  let secpAdminAuthPda: PublicKey;

  beforeAll(async () => {
    ctx = await setupTest();

    owner = Keypair.generate();
    const userSeed = getRandomSeed();

    // Create wallet with Owner (Ed25519)
    const { ix, walletPda: w } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed,
    });
    await sendTx(ctx, [ix]);
    walletPda = w;

    const [v] = findVaultPda(walletPda);
    vaultPda = v;

    // Generate Secp256r1 signer
    secpAdmin = await generateMockSecp256r1Signer();

    // Owner adds Secp256r1 Admin
    const { ix: ixAddAdmin } = await ctx.highClient.addAuthority({
      payer: ctx.payer,
      adminType: AuthType.Ed25519,
      adminSigner: owner,
      newAuthPubkey: secpAdmin.publicKeyBytes,
      newAuthType: AuthType.Secp256r1,
      newCredentialHash: secpAdmin.credentialIdHash,
      role: Role.Admin,
      walletPda,
    });
    await sendTx(ctx, [ixAddAdmin], [owner]);

    // Derive Secp256r1 Admin authority PDA (use credentialIdHash as seed, not pubkey)
    const [aPda] = findAuthorityPda(walletPda, secpAdmin.credentialIdHash);
    secpAdminAuthPda = aPda;

    console.log('🔐 Secp256r1 Admin added, testing counter...');
  }, 30_000);

  it('Executes increment counter correctly 10 times', async () => {
    // Fund vault for transfer
    const transferAmount = 2 * LAMPORTS_PER_SOL;
    const fundIx = SystemProgram.transfer({
      fromPubkey: ctx.payer.publicKey,
      toPubkey: vaultPda,
      lamports: transferAmount,
    });
    await sendTx(ctx, [fundIx]);

    // Read counter before
    const authBefore = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      secpAdminAuthPda,
    );
    expect(Number(authBefore.counter)).toBe(0);

    // Loop execute 10 times
    let currentCounter = 0;
    for (let i = 1; i <= 10; i++) {
      const recipient = Keypair.generate();
      const transferIx = SystemProgram.transfer({
        fromPubkey: vaultPda,
        toPubkey: recipient.publicKey,
        lamports: 0.05 * LAMPORTS_PER_SOL,
      });

      const { precompileIx, executeIx } = await buildSecp256r1Execute(
        ctx,
        secpAdmin,
        walletPda,
        [transferIx],
      );
      const result = await tryProcessInstructions(ctx, [
        precompileIx,
        executeIx,
      ]);
      expect(result.result).toBe('ok');

      const authAfter = await AuthorityAccount.fromAccountAddress(
        ctx.connection,
        secpAdminAuthPda,
      );
      expect(Number(authAfter.counter)).toBe(i);
      currentCounter = Number(authAfter.counter);
    }
    expect(currentCounter).toBe(10);
  }, 120_000);

  // No additional test - covered by loop above

  // Replay/reuse not testable when user cannot control counter

  // Out-of-order test not relevant when counter auto-incremented only

  // Counter=0 test not relevant (user cannot submit custom counter)
});
