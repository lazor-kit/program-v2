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
  AUTH_TYPE_ED25519,
  AUTH_TYPE_SECP256R1,
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
    let ownerKp: Keypair;
    let ownerAuthorityPda: PublicKey;

    beforeAll(async () => {
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

      // Fund the vault so it can transfer SOL
      const sig = await ctx.connection.requestAirdrop(
        result.vaultPda,
        2 * LAMPORTS_PER_SOL,
      );
      await ctx.connection.confirmTransaction(sig, 'confirmed');
    });

    it('builds a valid Ed25519 execute instruction', async () => {
      // System transfer: discriminator 2, then lamports u64 LE
      const transferData = Buffer.alloc(12);
      transferData.writeUInt32LE(2, 0); // Transfer
      transferData.writeBigUInt64LE(100_000n, 4);

      const ix = client.executeEd25519({
        payer: ctx.payer.publicKey,
        walletPda,
        authorityPda: ownerAuthorityPda,
        compactInstructions: [
          {
            programIdIndex: 4, // SystemProgram
            accountIndexes: [3, 5], // vault (from), recipient (to)
            data: new Uint8Array(transferData),
          },
        ],
        remainingAccounts: [
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
          {
            pubkey: Keypair.generate().publicKey,
            isSigner: false,
            isWritable: true,
          },
        ],
      });

      expect(ix.data.length).toBeGreaterThan(0);
      expect(ix.keys.length).toBeGreaterThanOrEqual(4);
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

      const result = client.createWalletSecp256r1({
        payer: ctx.payer.publicKey,
        userSeed,
        credentialIdHash: ownerKey.credentialIdHash,
        compressedPubkey: ownerKey.publicKeyBytes,
        rpId: ownerKey.rpId,
      });
      walletPda = result.walletPda;
      vaultPda = result.vaultPda;
      ownerAuthorityPda = result.authorityPda;

      await sendTx(ctx, [result.ix]);

      // Fund the vault
      const sig = await ctx.connection.requestAirdrop(
        vaultPda,
        2 * LAMPORTS_PER_SOL,
      );
      await ctx.connection.confirmTransaction(sig, 'confirmed');
    });

    it('executes a SOL transfer with low-level executeSecp256r1()', async () => {
      const recipient = Keypair.generate().publicKey;
      const signer = createMockSigner(ownerKey);

      // System transfer: discriminator 2, then lamports u64 LE
      const transferData = Buffer.alloc(12);
      transferData.writeUInt32LE(2, 0);
      transferData.writeBigUInt64LE(1_000_000n, 4);

      const { ix, precompileIx } = await client.executeSecp256r1({
        payer: ctx.payer.publicKey,
        walletPda,
        authorityPda: ownerAuthorityPda,
        signer,
        compactInstructions: [
          {
            programIdIndex: 5,
            accountIndexes: [3, 6],
            data: new Uint8Array(transferData),
          },
        ],
        remainingAccounts: [
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          { pubkey: recipient, isSigner: false, isWritable: true },
        ],
      });

      const balanceBefore = await ctx.connection.getBalance(recipient);
      await sendTx(ctx, [precompileIx, ix]);
      const balanceAfter = await ctx.connection.getBalance(recipient);

      expect(balanceAfter - balanceBefore).toBe(1_000_000);
    });

    it('executes a SOL transfer with transferSol()', async () => {
      const recipient = Keypair.generate().publicKey;
      const signer = createMockSigner(ownerKey);

      const ixs = await client.transferSol({
        payer: ctx.payer.publicKey,
        walletPda,
        signer,
        recipient,
        lamports: 1_000_000n,
      });

      const balanceBefore = await ctx.connection.getBalance(recipient);
      await sendTx(ctx, ixs);
      const balanceAfter = await ctx.connection.getBalance(recipient);

      expect(balanceAfter - balanceBefore).toBe(1_000_000);
    });

    it('executes arbitrary instructions with execute()', async () => {
      const recipient = Keypair.generate().publicKey;
      const signer = createMockSigner(ownerKey);
      const [vault] = client.findVault(walletPda);

      const ixs = await client.execute({
        payer: ctx.payer.publicKey,
        walletPda,
        signer,
        instructions: [
          SystemProgram.transfer({
            fromPubkey: vault,
            toPubkey: recipient,
            lamports: 1_000_000,
          }),
        ],
      });

      const balanceBefore = await ctx.connection.getBalance(recipient);
      await sendTx(ctx, ixs);
      const balanceAfter = await ctx.connection.getBalance(recipient);

      expect(balanceAfter - balanceBefore).toBe(1_000_000);
    });

    it('increments counter after successful execute', async () => {
      const recipient = Keypair.generate().publicKey;
      const signer = createMockSigner(ownerKey);

      const ixs = await client.transferSol({
        payer: ctx.payer.publicKey,
        walletPda,
        signer,
        recipient,
        lamports: 1_000_000n,
      });

      await sendTx(ctx, ixs);

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
