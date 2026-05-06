import { describe, it, expect, beforeAll } from 'vitest';
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  type AccountMeta,
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
import { generateMockSecp256r1Key, signSecp256r1 } from './secp256r1Utils';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  createCreateWalletIx,
  createExecuteIx,
  packCompactInstructions,
  computeAccountsHash,
  AUTH_TYPE_SECP256R1,
  DISC_EXECUTE,
} from '@lazorkit/sdk-legacy';

describe('Replay Prevention (Odometer)', () => {
  let ctx: TestContext;
  let walletPda: PublicKey;
  let vaultPda: PublicKey;
  let ownerKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
  let ownerAuthorityPda: PublicKey;

  // Helper to build a simple transfer execute instruction
  const compactIxDef = [
    {
      programIdIndex: 5,
      accountIndexes: [3, 6],
      data: new Uint8Array(
        (() => {
          const d = Buffer.alloc(12);
          d.writeUInt32LE(2, 0);
          d.writeBigUInt64LE(1_000_000n, 4);
          return d;
        })(),
      ),
    },
  ];

  function buildTransferPacked() {
    return packCompactInstructions(compactIxDef);
  }

  async function buildExecuteIx(counter: number, packed: Uint8Array) {
    const slot = await getSlot(ctx);
    const recipient = Keypair.generate().publicKey;

    // On-chain extends: signed_payload = compact_bytes + accounts_hash
    const allAccountMetas = [
      { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: false },
      { pubkey: walletPda, isSigner: false, isWritable: false },
      { pubkey: ownerAuthorityPda, isSigner: false, isWritable: true },
      { pubkey: vaultPda, isSigner: false, isWritable: true },
      { pubkey: PublicKey.default, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: recipient, isSigner: false, isWritable: true },
    ];
    const accountsHash = computeAccountsHash(allAccountMetas, compactIxDef);
    const signedPayload = Buffer.concat([packed, accountsHash]);

    const { authPayload, precompileIx } = await signSecp256r1({
      key: ownerKey,
      discriminator: new Uint8Array([DISC_EXECUTE]),
      signedPayload,
      slot,
      counter,
      payer: ctx.payer.publicKey,
      sysvarIxIndex: 4,
    });

    const ix = createExecuteIx({
      payer: ctx.payer.publicKey,
      walletPda,
      authorityPda: ownerAuthorityPda,
      vaultPda,
      packedInstructions: packed,
      authPayload,
      remainingAccounts: [
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: recipient, isSigner: false, isWritable: true },
      ],
      programId: PROGRAM_ID,
    });

    return { precompileIx, ix };
  }

  beforeAll(async () => {
    ctx = await setupTest();

    ownerKey = await generateMockSecp256r1Key();
    const userSeed = crypto.randomBytes(32);

    [walletPda] = findWalletPda(userSeed, PROGRAM_ID);
    [vaultPda] = findVaultPda(walletPda, PROGRAM_ID);
    const [authPda, authBump] = findAuthorityPda(
      walletPda,
      ownerKey.credentialIdHash,
      PROGRAM_ID,
    );
    ownerAuthorityPda = authPda;

    await sendTx(ctx, [
      createCreateWalletIx({
        payer: ctx.payer.publicKey,
        walletPda,
        vaultPda,
        authorityPda: authPda,
        userSeed,
        authType: AUTH_TYPE_SECP256R1,
        authBump,
        credentialOrPubkey: ownerKey.credentialIdHash,
        secp256r1Pubkey: ownerKey.publicKeyBytes,
        rpId: ownerKey.rpId,
      programId: PROGRAM_ID,
      }),
    ]);

    // Fund vault
    const sig = await ctx.connection.requestAirdrop(
      vaultPda,
      5 * LAMPORTS_PER_SOL,
    );
    await ctx.connection.confirmTransaction(sig, 'confirmed');
  });

  it('accepts counter=1 for fresh authority (stored=0)', async () => {
    const packed = buildTransferPacked();
    const { precompileIx, ix } = await buildExecuteIx(1, packed);
    await sendTx(ctx, [precompileIx, ix]);

    // Verify counter is now 1
    const info = await ctx.connection.getAccountInfo(ownerAuthorityPda);
    const view = new DataView(info!.data.buffer, info!.data.byteOffset);
    expect(view.getUint32(8, true)).toBe(1);
  });

  it('rejects same counter=1 replay (SignatureReused 3006)', async () => {
    const packed = buildTransferPacked();
    // Counter is now 1 on-chain, submitting 1 again should fail
    await sendTxExpectError(
      ctx,
      [
        (await buildExecuteIx(1, packed)).precompileIx,
        (await buildExecuteIx(1, packed)).ix,
      ],
      [],
      3006, // SignatureReused
    );
  });

  it('rejects counter=0 (behind stored)', async () => {
    const packed = buildTransferPacked();
    await sendTxExpectError(
      ctx,
      [
        (await buildExecuteIx(0, packed)).precompileIx,
        (await buildExecuteIx(0, packed)).ix,
      ],
      [],
      3006,
    );
  });

  it('rejects counter=5 (skipping ahead)', async () => {
    const packed = buildTransferPacked();
    // Stored counter is 1, expected next is 2, submitting 5 should fail
    await sendTxExpectError(
      ctx,
      [
        (await buildExecuteIx(5, packed)).precompileIx,
        (await buildExecuteIx(5, packed)).ix,
      ],
      [],
      3006,
    );
  });

  it('accepts sequential counter 2, 3, 4', async () => {
    for (const c of [2, 3, 4]) {
      const packed = buildTransferPacked();
      const { precompileIx, ix } = await buildExecuteIx(c, packed);
      await sendTx(ctx, [precompileIx, ix]);
    }

    // Verify counter is now 4
    const info = await ctx.connection.getAccountInfo(ownerAuthorityPda);
    const view = new DataView(info!.data.buffer, info!.data.byteOffset);
    expect(view.getUint32(8, true)).toBe(4);
  });

  it('rejects stale counter after sequential ops', async () => {
    const packed = buildTransferPacked();
    // Counter is 4, submitting 3 should fail
    await sendTxExpectError(
      ctx,
      [
        (await buildExecuteIx(3, packed)).precompileIx,
        (await buildExecuteIx(3, packed)).ix,
      ],
      [],
      3006,
    );
  });
});
