import { describe, it, expect, beforeAll } from 'vitest';
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import * as crypto from 'crypto';
import { setupTest, sendTx, type TestContext } from './common';
import { generateMockSecp256r1Key, createMockSigner } from './secp256r1Utils';
import {
  LazorKitClient,
  ed25519,
  secp256r1,
} from '../../sdk/solita-client/src';

describe('Execute', () => {
  let ctx: TestContext;
  let client: LazorKitClient;

  beforeAll(async () => {
    ctx = await setupTest();
    client = new LazorKitClient(ctx.connection);
  });

  describe('Ed25519 Execute', () => {
    let walletPda: PublicKey;
    let vaultPda: PublicKey;
    let ownerKp: Keypair;
    let ownerAuthorityPda: PublicKey;

    beforeAll(async () => {
      ownerKp = Keypair.generate();
      const userSeed = crypto.randomBytes(32);

      const result = client.createWallet({
        payer: ctx.payer.publicKey,
        userSeed,
        owner: { type: 'ed25519', publicKey: ownerKp.publicKey },
      });
      walletPda = result.walletPda;
      vaultPda = result.vaultPda;
      ownerAuthorityPda = result.authorityPda;

      await sendTx(ctx, result.instructions);

      // Fund the vault so it can transfer SOL
      const sig = await ctx.connection.requestAirdrop(
        result.vaultPda,
        2 * LAMPORTS_PER_SOL,
      );
      await ctx.connection.confirmTransaction(sig, 'confirmed');
    });

    it('executes a SOL transfer via execute()', async () => {
      const recipient = Keypair.generate().publicKey;

      const { instructions } = await client.execute({
        payer: ctx.payer.publicKey,
        walletPda,
        signer: ed25519(ownerKp.publicKey, ownerAuthorityPda),
        instructions: [
          SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: recipient,
            lamports: 1_000_000,
          }),
        ],
      });

      const balanceBefore = await ctx.connection.getBalance(recipient);
      await sendTx(ctx, instructions, [ownerKp]);
      const balanceAfter = await ctx.connection.getBalance(recipient);

      expect(balanceAfter - balanceBefore).toBe(1_000_000);
    });
  });

  describe('Secp256r1 Execute', () => {
    let walletPda: PublicKey;
    let vaultPda: PublicKey;
    let ownerKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
    let ownerAuthorityPda: PublicKey;

    beforeAll(async () => {
      ownerKey = await generateMockSecp256r1Key();
      const userSeed = crypto.randomBytes(32);

      const result = client.createWallet({
        payer: ctx.payer.publicKey,
        userSeed,
        owner: {
          type: 'secp256r1',
          credentialIdHash: ownerKey.credentialIdHash,
          compressedPubkey: ownerKey.publicKeyBytes,
          rpId: ownerKey.rpId,
        },
      });
      walletPda = result.walletPda;
      vaultPda = result.vaultPda;
      ownerAuthorityPda = result.authorityPda;

      await sendTx(ctx, result.instructions);

      // Fund the vault
      const sig = await ctx.connection.requestAirdrop(
        vaultPda,
        2 * LAMPORTS_PER_SOL,
      );
      await ctx.connection.confirmTransaction(sig, 'confirmed');
    });

    it('executes a SOL transfer via execute()', async () => {
      const recipient = Keypair.generate().publicKey;
      const signer = createMockSigner(ownerKey);

      const { instructions } = await client.execute({
        payer: ctx.payer.publicKey,
        walletPda,
        signer: secp256r1(signer, { authorityPda: ownerAuthorityPda }),
        instructions: [
          SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: recipient,
            lamports: 1_000_000,
          }),
        ],
      });

      const balanceBefore = await ctx.connection.getBalance(recipient);
      await sendTx(ctx, instructions);
      const balanceAfter = await ctx.connection.getBalance(recipient);

      expect(balanceAfter - balanceBefore).toBe(1_000_000);
    });

    it('executes a SOL transfer with transferSol()', async () => {
      const recipient = Keypair.generate().publicKey;
      const signer = createMockSigner(ownerKey);

      const { instructions } = await client.transferSol({
        payer: ctx.payer.publicKey,
        walletPda,
        signer: secp256r1(signer, { authorityPda: ownerAuthorityPda }),
        recipient,
        lamports: 1_000_000n,
      });

      const balanceBefore = await ctx.connection.getBalance(recipient);
      await sendTx(ctx, instructions);
      const balanceAfter = await ctx.connection.getBalance(recipient);

      expect(balanceAfter - balanceBefore).toBe(1_000_000);
    });

    it('executes arbitrary instructions with execute()', async () => {
      const recipient = Keypair.generate().publicKey;
      const signer = createMockSigner(ownerKey);

      const { instructions } = await client.execute({
        payer: ctx.payer.publicKey,
        walletPda,
        signer: secp256r1(signer, { authorityPda: ownerAuthorityPda }),
        instructions: [
          SystemProgram.transfer({
            fromPubkey: vaultPda,
            toPubkey: recipient,
            lamports: 1_000_000,
          }),
        ],
      });

      const balanceBefore = await ctx.connection.getBalance(recipient);
      await sendTx(ctx, instructions);
      const balanceAfter = await ctx.connection.getBalance(recipient);

      expect(balanceAfter - balanceBefore).toBe(1_000_000);
    });

    it('increments counter after successful execute', async () => {
      const recipient = Keypair.generate().publicKey;
      const signer = createMockSigner(ownerKey);

      const { instructions } = await client.transferSol({
        payer: ctx.payer.publicKey,
        walletPda,
        signer: secp256r1(signer, { authorityPda: ownerAuthorityPda }),
        recipient,
        lamports: 1_000_000n,
      });

      await sendTx(ctx, instructions);

      // Verify counter is now 4 (three Secp256r1 executes above + this one)
      const authority = await ctx.connection.getAccountInfo(ownerAuthorityPda);
      const view = new DataView(
        authority!.data.buffer,
        authority!.data.byteOffset,
      );
      const counter = view.getUint32(8, true);
      expect(counter).toBe(4);
    });
  });
});
