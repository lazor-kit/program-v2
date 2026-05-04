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
import { generateMockSecp256r1Key, signSecp256r1 } from './secp256r1Utils';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findDeferredExecPda,
  createCreateWalletIx,
  createAuthorizeIx,
  createExecuteDeferredIx,
  createReclaimDeferredIx,
  packCompactInstructions,
  computeAccountsHash,
  computeInstructionsHash,
  buildAuthPayload,
  buildSecp256r1Challenge,
  AUTH_TYPE_SECP256R1,
  DISC_AUTHORIZE,
} from '@lazorkit/sdk-legacy';

describe('Deferred Execution', () => {
  let ctx: TestContext;

  beforeAll(async () => {
    ctx = await setupTest();
  });

  describe('Happy Path', () => {
    let walletPda: PublicKey;
    let vaultPda: PublicKey;
    let ownerKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
    let ownerAuthorityPda: PublicKey;

    beforeAll(async () => {
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

      // Fund the vault
      const sig = await ctx.connection.requestAirdrop(
        vaultPda,
        5 * LAMPORTS_PER_SOL,
      );
      await ctx.connection.confirmTransaction(sig, 'confirmed');
    });

    it('authorizes and executes a single SOL transfer via deferred', async () => {
      const recipient = Keypair.generate().publicKey;
      const slot = await getSlot(ctx);

      // Build a SOL transfer as compact instruction
      const transferData = Buffer.alloc(12);
      transferData.writeUInt32LE(2, 0); // System Transfer
      transferData.writeBigUInt64LE(BigInt(LAMPORTS_PER_SOL), 4);

      // Compact instruction indices are relative to tx2 account layout:
      //   0: payer, 1: wallet, 2: vault, 3: deferred, 4: refundDest
      //   5: SystemProgram, 6: recipient
      const compactIxs = [
        {
          programIdIndex: 5,
          accountIndexes: [2, 6], // vault (from), recipient (to)
          data: new Uint8Array(transferData),
        },
      ];

      // Compute hashes
      const instructionsHash = computeInstructionsHash(compactIxs);
      const tx2AccountMetas = [
        { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: walletPda, isSigner: false, isWritable: false },
        { pubkey: vaultPda, isSigner: false, isWritable: true },
        { pubkey: PublicKey.default, isSigner: false, isWritable: true }, // deferred placeholder
        { pubkey: ctx.payer.publicKey, isSigner: false, isWritable: true }, // refund dest
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: recipient, isSigner: false, isWritable: true },
      ];
      const accountsHash = computeAccountsHash(tx2AccountMetas, compactIxs);

      // Build signed_payload = instructions_hash || accounts_hash
      const signedPayload = Buffer.concat([instructionsHash, accountsHash]);

      // Sign with Secp256r1
      const { authPayload, precompileIx } = await signSecp256r1({
        key: ownerKey,
        discriminator: new Uint8Array([DISC_AUTHORIZE]),
        signedPayload,
        slot,
        counter: 1,
        payer: ctx.payer.publicKey,
        sysvarIxIndex: 6, // index 6 in Authorize tx accounts
      });

      // Derive deferred PDA (counter = 1)
      const [deferredExecPda] = findDeferredExecPda(
        walletPda,
        ownerAuthorityPda,
        1,
      PROGRAM_ID,
    );

      // === TX1: Authorize ===
      const authorizeIx = createAuthorizeIx({
        payer: ctx.payer.publicKey,
        walletPda,
        authorityPda: ownerAuthorityPda,
        deferredExecPda,
        instructionsHash,
        accountsHash,
        expiryOffset: 300,
        authPayload,
      programId: PROGRAM_ID,
      });

      await sendTx(ctx, [precompileIx, authorizeIx]);

      // Verify DeferredExec account was created
      const deferredAccount =
        await ctx.connection.getAccountInfo(deferredExecPda);
      expect(deferredAccount).not.toBeNull();
      expect(deferredAccount!.data.length).toBe(176);
      expect(deferredAccount!.data[0]).toBe(4); // DeferredExec discriminator

      // === TX2: ExecuteDeferred ===
      const packed = packCompactInstructions(compactIxs);
      const executeDeferredIx = createExecuteDeferredIx({
        payer: ctx.payer.publicKey,
        walletPda,
        vaultPda,
        deferredExecPda,
        refundDestination: ctx.payer.publicKey,
        packedInstructions: packed,
        remainingAccounts: [
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
          { pubkey: recipient, isSigner: false, isWritable: true },
        ],
      programId: PROGRAM_ID,
      });

      const balanceBefore = await ctx.connection.getBalance(recipient);
      await sendTx(ctx, [executeDeferredIx]);
      const balanceAfter = await ctx.connection.getBalance(recipient);

      expect(balanceAfter - balanceBefore).toBe(LAMPORTS_PER_SOL);

      // Verify DeferredExec account was closed
      const deferredAfter =
        await ctx.connection.getAccountInfo(deferredExecPda);
      expect(deferredAfter).toBeNull();
    });

    it('authorizes and executes multiple SOL transfers via deferred', async () => {
      const recipient1 = Keypair.generate().publicKey;
      const recipient2 = Keypair.generate().publicKey;
      const recipient3 = Keypair.generate().publicKey;
      const slot = await getSlot(ctx);

      // 3 SOL transfers — simulates a complex multi-instruction payload
      const makeTransferData = (amount: bigint) => {
        const buf = Buffer.alloc(12);
        buf.writeUInt32LE(2, 0);
        buf.writeBigUInt64LE(amount, 4);
        return new Uint8Array(buf);
      };

      // tx2 layout:
      //   0: payer, 1: wallet, 2: vault, 3: deferred, 4: refundDest
      //   5: SystemProgram, 6: recipient1, 7: recipient2, 8: recipient3
      const compactIxs = [
        {
          programIdIndex: 5,
          accountIndexes: [2, 6],
          data: makeTransferData(BigInt(LAMPORTS_PER_SOL)),
        },
        {
          programIdIndex: 5,
          accountIndexes: [2, 7],
          data: makeTransferData(BigInt(LAMPORTS_PER_SOL)),
        },
        {
          programIdIndex: 5,
          accountIndexes: [2, 8],
          data: makeTransferData(BigInt(LAMPORTS_PER_SOL)),
        },
      ];

      const instructionsHash = computeInstructionsHash(compactIxs);
      const tx2AccountMetas = [
        { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: walletPda, isSigner: false, isWritable: false },
        { pubkey: vaultPda, isSigner: false, isWritable: true },
        { pubkey: PublicKey.default, isSigner: false, isWritable: true },
        { pubkey: ctx.payer.publicKey, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: recipient1, isSigner: false, isWritable: true },
        { pubkey: recipient2, isSigner: false, isWritable: true },
        { pubkey: recipient3, isSigner: false, isWritable: true },
      ];
      const accountsHash = computeAccountsHash(tx2AccountMetas, compactIxs);
      const signedPayload = Buffer.concat([instructionsHash, accountsHash]);

      // Counter is now 2 (after first test incremented to 1)
      const { authPayload, precompileIx } = await signSecp256r1({
        key: ownerKey,
        discriminator: new Uint8Array([DISC_AUTHORIZE]),
        signedPayload,
        slot,
        counter: 2,
        payer: ctx.payer.publicKey,
        sysvarIxIndex: 6,
      });

      const [deferredExecPda] = findDeferredExecPda(
        walletPda,
        ownerAuthorityPda,
        2,
      PROGRAM_ID,
    );

      // TX1: Authorize
      await sendTx(ctx, [
        precompileIx,
        createAuthorizeIx({
          payer: ctx.payer.publicKey,
          walletPda,
          authorityPda: ownerAuthorityPda,
          deferredExecPda,
          instructionsHash,
          accountsHash,
          expiryOffset: 300,
          authPayload,
      programId: PROGRAM_ID,
        }),
      ]);

      // TX2: ExecuteDeferred
      const packed = packCompactInstructions(compactIxs);
      await sendTx(ctx, [
        createExecuteDeferredIx({
          payer: ctx.payer.publicKey,
          walletPda,
          vaultPda,
          deferredExecPda,
          refundDestination: ctx.payer.publicKey,
          packedInstructions: packed,
          remainingAccounts: [
            {
              pubkey: SystemProgram.programId,
              isSigner: false,
              isWritable: false,
            },
            { pubkey: recipient1, isSigner: false, isWritable: true },
            { pubkey: recipient2, isSigner: false, isWritable: true },
            { pubkey: recipient3, isSigner: false, isWritable: true },
          ],
      programId: PROGRAM_ID,
        }),
      ]);

      // Verify all transfers
      const bal1 = await ctx.connection.getBalance(recipient1);
      const bal2 = await ctx.connection.getBalance(recipient2);
      const bal3 = await ctx.connection.getBalance(recipient3);
      expect(bal1).toBe(LAMPORTS_PER_SOL);
      expect(bal2).toBe(LAMPORTS_PER_SOL);
      expect(bal3).toBe(LAMPORTS_PER_SOL);
    });
  });

  describe('Security', () => {
    let walletPda: PublicKey;
    let vaultPda: PublicKey;
    let ownerKey: Awaited<ReturnType<typeof generateMockSecp256r1Key>>;
    let ownerAuthorityPda: PublicKey;

    beforeAll(async () => {
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

      const sig = await ctx.connection.requestAirdrop(
        vaultPda,
        5 * LAMPORTS_PER_SOL,
      );
      await ctx.connection.confirmTransaction(sig, 'confirmed');
    });

    it('rejects ExecuteDeferred with wrong instructions (hash mismatch)', async () => {
      const recipient = Keypair.generate().publicKey;
      const slot = await getSlot(ctx);

      const transferData = Buffer.alloc(12);
      transferData.writeUInt32LE(2, 0);
      transferData.writeBigUInt64LE(BigInt(LAMPORTS_PER_SOL), 4);

      // Authorize with 100k transfer
      const compactIxs = [
        {
          programIdIndex: 5,
          accountIndexes: [2, 6],
          data: new Uint8Array(transferData),
        },
      ];

      const instructionsHash = computeInstructionsHash(compactIxs);
      const tx2AccountMetas = [
        { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: walletPda, isSigner: false, isWritable: false },
        { pubkey: vaultPda, isSigner: false, isWritable: true },
        { pubkey: PublicKey.default, isSigner: false, isWritable: true },
        { pubkey: ctx.payer.publicKey, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: recipient, isSigner: false, isWritable: true },
      ];
      const accountsHash = computeAccountsHash(tx2AccountMetas, compactIxs);
      const signedPayload = Buffer.concat([instructionsHash, accountsHash]);

      const { authPayload, precompileIx } = await signSecp256r1({
        key: ownerKey,
        discriminator: new Uint8Array([DISC_AUTHORIZE]),
        signedPayload,
        slot,
        counter: 1,
        payer: ctx.payer.publicKey,
        sysvarIxIndex: 6,
      });

      const [deferredExecPda] = findDeferredExecPda(
        walletPda,
        ownerAuthorityPda,
        1,
      PROGRAM_ID,
    );

      // TX1: Authorize
      await sendTx(ctx, [
        precompileIx,
        createAuthorizeIx({
          payer: ctx.payer.publicKey,
          walletPda,
          authorityPda: ownerAuthorityPda,
          deferredExecPda,
          instructionsHash,
          accountsHash,
          expiryOffset: 300,
          authPayload,
      programId: PROGRAM_ID,
        }),
      ]);

      // TX2: Try to execute with DIFFERENT instructions (1M instead of 100k)
      const wrongTransferData = Buffer.alloc(12);
      wrongTransferData.writeUInt32LE(2, 0);
      wrongTransferData.writeBigUInt64LE(BigInt(2 * LAMPORTS_PER_SOL), 4); // 2 SOL instead of 1 SOL

      const wrongCompactIxs = [
        {
          programIdIndex: 5,
          accountIndexes: [2, 6],
          data: new Uint8Array(wrongTransferData),
        },
      ];
      const wrongPacked = packCompactInstructions(wrongCompactIxs);

      await sendTxExpectError(
        ctx,
        [
          createExecuteDeferredIx({
            payer: ctx.payer.publicKey,
            walletPda,
            vaultPda,
            deferredExecPda,
            refundDestination: ctx.payer.publicKey,
            packedInstructions: wrongPacked,
            remainingAccounts: [
              {
                pubkey: SystemProgram.programId,
                isSigner: false,
                isWritable: false,
              },
              { pubkey: recipient, isSigner: false, isWritable: true },
            ],
      programId: PROGRAM_ID,
          }),
        ],
        [],
        3015,
      ); // DeferredHashMismatch

      // Now execute with correct instructions to clean up
      const correctPacked = packCompactInstructions(compactIxs);
      await sendTx(ctx, [
        createExecuteDeferredIx({
          payer: ctx.payer.publicKey,
          walletPda,
          vaultPda,
          deferredExecPda,
          refundDestination: ctx.payer.publicKey,
          packedInstructions: correctPacked,
          remainingAccounts: [
            {
              pubkey: SystemProgram.programId,
              isSigner: false,
              isWritable: false,
            },
            { pubkey: recipient, isSigner: false, isWritable: true },
          ],
      programId: PROGRAM_ID,
        }),
      ]);
    });

    it('rejects double execution (account closed after first execute)', async () => {
      const recipient = Keypair.generate().publicKey;
      const slot = await getSlot(ctx);

      const transferData = Buffer.alloc(12);
      transferData.writeUInt32LE(2, 0);
      transferData.writeBigUInt64LE(BigInt(LAMPORTS_PER_SOL), 4);

      const compactIxs = [
        {
          programIdIndex: 5,
          accountIndexes: [2, 6],
          data: new Uint8Array(transferData),
        },
      ];

      const instructionsHash = computeInstructionsHash(compactIxs);
      const tx2AccountMetas = [
        { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: walletPda, isSigner: false, isWritable: false },
        { pubkey: vaultPda, isSigner: false, isWritable: true },
        { pubkey: PublicKey.default, isSigner: false, isWritable: true },
        { pubkey: ctx.payer.publicKey, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: recipient, isSigner: false, isWritable: true },
      ];
      const accountsHash = computeAccountsHash(tx2AccountMetas, compactIxs);
      const signedPayload = Buffer.concat([instructionsHash, accountsHash]);

      const { authPayload, precompileIx } = await signSecp256r1({
        key: ownerKey,
        discriminator: new Uint8Array([DISC_AUTHORIZE]),
        signedPayload,
        slot,
        counter: 2,
        payer: ctx.payer.publicKey,
        sysvarIxIndex: 6,
      });

      const [deferredExecPda] = findDeferredExecPda(
        walletPda,
        ownerAuthorityPda,
        2,
      PROGRAM_ID,
    );

      // TX1: Authorize
      await sendTx(ctx, [
        precompileIx,
        createAuthorizeIx({
          payer: ctx.payer.publicKey,
          walletPda,
          authorityPda: ownerAuthorityPda,
          deferredExecPda,
          instructionsHash,
          accountsHash,
          expiryOffset: 300,
          authPayload,
      programId: PROGRAM_ID,
        }),
      ]);

      // TX2: Execute (should succeed)
      const packed = packCompactInstructions(compactIxs);
      const executeDeferredIx = createExecuteDeferredIx({
        payer: ctx.payer.publicKey,
        walletPda,
        vaultPda,
        deferredExecPda,
        refundDestination: ctx.payer.publicKey,
        packedInstructions: packed,
        remainingAccounts: [
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
          { pubkey: recipient, isSigner: false, isWritable: true },
        ],
      programId: PROGRAM_ID,
      });

      await sendTx(ctx, [executeDeferredIx]);

      // TX2 again: should fail (account closed)
      await sendTxExpectError(ctx, [
        createExecuteDeferredIx({
          payer: ctx.payer.publicKey,
          walletPda,
          vaultPda,
          deferredExecPda,
          refundDestination: ctx.payer.publicKey,
          packedInstructions: packed,
          remainingAccounts: [
            {
              pubkey: SystemProgram.programId,
              isSigner: false,
              isWritable: false,
            },
            { pubkey: recipient, isSigner: false, isWritable: true },
          ],
      programId: PROGRAM_ID,
        }),
      ]);
    });

    it('rejects reclaim before expiry', async () => {
      const recipient = Keypair.generate().publicKey;
      const slot = await getSlot(ctx);

      const transferData = Buffer.alloc(12);
      transferData.writeUInt32LE(2, 0);
      transferData.writeBigUInt64LE(BigInt(LAMPORTS_PER_SOL), 4);

      const compactIxs = [
        {
          programIdIndex: 5,
          accountIndexes: [2, 6],
          data: new Uint8Array(transferData),
        },
      ];

      const instructionsHash = computeInstructionsHash(compactIxs);
      const tx2AccountMetas = [
        { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: walletPda, isSigner: false, isWritable: false },
        { pubkey: vaultPda, isSigner: false, isWritable: true },
        { pubkey: PublicKey.default, isSigner: false, isWritable: true },
        { pubkey: ctx.payer.publicKey, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: recipient, isSigner: false, isWritable: true },
      ];
      const accountsHash = computeAccountsHash(tx2AccountMetas, compactIxs);
      const signedPayload = Buffer.concat([instructionsHash, accountsHash]);

      const { authPayload, precompileIx } = await signSecp256r1({
        key: ownerKey,
        discriminator: new Uint8Array([DISC_AUTHORIZE]),
        signedPayload,
        slot,
        counter: 3,
        payer: ctx.payer.publicKey,
        sysvarIxIndex: 6,
      });

      const [deferredExecPda] = findDeferredExecPda(
        walletPda,
        ownerAuthorityPda,
        3,
      PROGRAM_ID,
    );

      // TX1: Authorize with max expiry
      await sendTx(ctx, [
        precompileIx,
        createAuthorizeIx({
          payer: ctx.payer.publicKey,
          walletPda,
          authorityPda: ownerAuthorityPda,
          deferredExecPda,
          instructionsHash,
          accountsHash,
          expiryOffset: 9000, // ~1 hour
          authPayload,
      programId: PROGRAM_ID,
        }),
      ]);

      // Try to reclaim immediately (should fail — not expired yet)
      await sendTxExpectError(
        ctx,
        [
          createReclaimDeferredIx({
            payer: ctx.payer.publicKey,
            deferredExecPda,
            refundDestination: ctx.payer.publicKey,
      programId: PROGRAM_ID,
          }),
        ],
        [],
        3018,
      ); // DeferredAuthorizationNotExpired

      // Clean up: execute it
      const packed = packCompactInstructions(compactIxs);
      await sendTx(ctx, [
        createExecuteDeferredIx({
          payer: ctx.payer.publicKey,
          walletPda,
          vaultPda,
          deferredExecPda,
          refundDestination: ctx.payer.publicKey,
          packedInstructions: packed,
          remainingAccounts: [
            {
              pubkey: SystemProgram.programId,
              isSigner: false,
              isWritable: false,
            },
            { pubkey: recipient, isSigner: false, isWritable: true },
          ],
      programId: PROGRAM_ID,
        }),
      ]);
    });

    it('rejects reclaim from wrong payer', async () => {
      const recipient = Keypair.generate().publicKey;
      const slot = await getSlot(ctx);

      const transferData = Buffer.alloc(12);
      transferData.writeUInt32LE(2, 0);
      transferData.writeBigUInt64LE(BigInt(LAMPORTS_PER_SOL), 4);

      const compactIxs = [
        {
          programIdIndex: 5,
          accountIndexes: [2, 6],
          data: new Uint8Array(transferData),
        },
      ];

      const instructionsHash = computeInstructionsHash(compactIxs);
      const tx2AccountMetas = [
        { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: true },
        { pubkey: walletPda, isSigner: false, isWritable: false },
        { pubkey: vaultPda, isSigner: false, isWritable: true },
        { pubkey: PublicKey.default, isSigner: false, isWritable: true },
        { pubkey: ctx.payer.publicKey, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: recipient, isSigner: false, isWritable: true },
      ];
      const accountsHash = computeAccountsHash(tx2AccountMetas, compactIxs);
      const signedPayload = Buffer.concat([instructionsHash, accountsHash]);

      const { authPayload, precompileIx } = await signSecp256r1({
        key: ownerKey,
        discriminator: new Uint8Array([DISC_AUTHORIZE]),
        signedPayload,
        slot,
        counter: 4,
        payer: ctx.payer.publicKey,
        sysvarIxIndex: 6,
      });

      const [deferredExecPda] = findDeferredExecPda(
        walletPda,
        ownerAuthorityPda,
        4,
      PROGRAM_ID,
    );

      // TX1: Authorize with short expiry
      await sendTx(ctx, [
        precompileIx,
        createAuthorizeIx({
          payer: ctx.payer.publicKey,
          walletPda,
          authorityPda: ownerAuthorityPda,
          deferredExecPda,
          instructionsHash,
          accountsHash,
          expiryOffset: 10, // minimum expiry
          authPayload,
      programId: PROGRAM_ID,
        }),
      ]);

      // Create a different payer and try to reclaim
      const wrongPayer = Keypair.generate();
      const airdropSig = await ctx.connection.requestAirdrop(
        wrongPayer.publicKey,
        LAMPORTS_PER_SOL,
      );
      await ctx.connection.confirmTransaction(airdropSig, 'confirmed');

      // Even if expired, wrong payer should fail
      // Wait for expiry by sending some transactions to advance slots
      for (let i = 0; i < 5; i++) {
        await ctx.connection.requestAirdrop(Keypair.generate().publicKey, 1000);
      }

      await sendTxExpectError(
        ctx,
        [
          createReclaimDeferredIx({
            payer: wrongPayer.publicKey,
            deferredExecPda,
            refundDestination: wrongPayer.publicKey,
      programId: PROGRAM_ID,
          }),
        ],
        [wrongPayer],
        3017,
      ); // UnauthorizedReclaim

      // Clean up: execute with correct payer
      const packed = packCompactInstructions(compactIxs);
      // Wait for more slots just in case expiry hasn't passed
      // If expired, this will fail too — but it's fine since we proved the wrong payer was rejected
      try {
        await sendTx(ctx, [
          createExecuteDeferredIx({
            payer: ctx.payer.publicKey,
            walletPda,
            vaultPda,
            deferredExecPda,
            refundDestination: ctx.payer.publicKey,
            packedInstructions: packed,
            remainingAccounts: [
              {
                pubkey: SystemProgram.programId,
                isSigner: false,
                isWritable: false,
              },
              { pubkey: recipient, isSigner: false, isWritable: true },
            ],
      programId: PROGRAM_ID,
          }),
        ]);
      } catch {
        // May have expired — that's fine, reclaim with correct payer
        await sendTx(ctx, [
          createReclaimDeferredIx({
            payer: ctx.payer.publicKey,
            deferredExecPda,
            refundDestination: ctx.payer.publicKey,
      programId: PROGRAM_ID,
          }),
        ]);
      }
    });

    it('counter increments correctly across deferred and regular execute', async () => {
      // After 4 deferred authorizations, counter should be 4
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
