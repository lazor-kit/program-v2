import { describe, it, expect, beforeAll } from 'vitest';
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  type AccountMeta,
} from '@solana/web3.js';
import * as crypto from 'crypto';
import { setupTest, sendTx, getSlot, PROGRAM_ID, type TestContext } from './common';
import { generateMockSecp256r1Key, signSecp256r1 } from './secp256r1Utils';
import {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  createCreateWalletIx,
  createAddAuthorityIx,
  createExecuteIx,
  packCompactInstructions,
  computeAccountsHash,
  AUTH_TYPE_SECP256R1,
  AUTH_TYPE_ED25519,
  ROLE_ADMIN,
  ROLE_SPENDER,
  DISC_ADD_AUTHORITY,
  DISC_EXECUTE,
} from '@lazorkit/sdk-legacy';
import { AuthorityAccount } from '@lazorkit/sdk-legacy';

describe('Counter Edge Cases', () => {
  let ctx: TestContext;

  beforeAll(async () => {
    ctx = await setupTest();
  });

  it('counter persists across different instruction types', async () => {
    // Create wallet with Secp256r1 owner
    const ownerKey = await generateMockSecp256r1Key();
    const userSeed = crypto.randomBytes(32);

    const [walletPda] = findWalletPda(userSeed, PROGRAM_ID);
    const [vaultPda] = findVaultPda(walletPda, PROGRAM_ID);
    const [ownerAuthPda, authBump] = findAuthorityPda(
      walletPda,
      ownerKey.credentialIdHash,
      PROGRAM_ID,
    );

    await sendTx(ctx, [
      createCreateWalletIx({
        payer: ctx.payer.publicKey,
        walletPda,
        vaultPda,
        authorityPda: ownerAuthPda,
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
    const airdropSig = await ctx.connection.requestAirdrop(
      vaultPda,
      2 * LAMPORTS_PER_SOL,
    );
    await ctx.connection.confirmTransaction(airdropSig, 'confirmed');

    // Counter = 0 initially

    // 1. AddAuthority (counter becomes 1)
    const adminKp = Keypair.generate();
    const adminPubkey = adminKp.publicKey.toBytes();
    const [adminAuthPda] = findAuthorityPda(walletPda, adminPubkey, PROGRAM_ID);

    const slot1 = await getSlot(ctx);
    const dataPayload = Buffer.concat([
      Buffer.from([AUTH_TYPE_ED25519, ROLE_ADMIN]),
      Buffer.alloc(6),
      adminPubkey,
    ]);
    // On-chain extends: extended_data_payload = data_payload + payer.key()
    const signedPayload1 = Buffer.concat([
      dataPayload,
      ctx.payer.publicKey.toBuffer(),
    ]);

    const { authPayload: ap1, precompileIx: pi1 } = await signSecp256r1({
      key: ownerKey,
      discriminator: new Uint8Array([DISC_ADD_AUTHORITY]),
      signedPayload: signedPayload1,
      slot: slot1,
      counter: 1,
      payer: ctx.payer.publicKey,
      sysvarIxIndex: 6,
    });

    await sendTx(ctx, [
      pi1,
      createAddAuthorityIx({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: ownerAuthPda,
        newAuthorityPda: adminAuthPda,
        newType: AUTH_TYPE_ED25519,
        newRole: ROLE_ADMIN,
        credentialOrPubkey: adminPubkey,
        authPayload: ap1,
      programId: PROGRAM_ID,
      }),
    ]);

    // Verify counter = 1
    let auth = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      ownerAuthPda,
    );
    expect(Number(auth.counter)).toBe(1);

    // 2. Execute (counter becomes 2) — using same authority
    const slot2 = await getSlot(ctx);
    const transferData = Buffer.alloc(12);
    transferData.writeUInt32LE(2, 0);
    transferData.writeBigUInt64LE(1_000_000n, 4);

    const compactIxs = [
      {
        programIdIndex: 5,
        accountIndexes: [3, 6],
        data: new Uint8Array(transferData),
      },
    ];
    const packed = packCompactInstructions(compactIxs);

    const execRecipient = Keypair.generate().publicKey;
    // On-chain extends: signed_payload = compact_bytes + accounts_hash
    const allAccountMetas = [
      { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: false },
      { pubkey: walletPda, isSigner: false, isWritable: false },
      { pubkey: ownerAuthPda, isSigner: false, isWritable: true },
      { pubkey: vaultPda, isSigner: false, isWritable: true },
      { pubkey: PublicKey.default, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: execRecipient, isSigner: false, isWritable: true },
    ];
    const accountsHash = computeAccountsHash(allAccountMetas, compactIxs);
    const signedPayload2 = Buffer.concat([packed, accountsHash]);

    const { authPayload: ap2, precompileIx: pi2 } = await signSecp256r1({
      key: ownerKey,
      discriminator: new Uint8Array([DISC_EXECUTE]),
      signedPayload: signedPayload2,
      slot: slot2,
      counter: 2,
      payer: ctx.payer.publicKey,
      sysvarIxIndex: 4,
    });

    await sendTx(ctx, [
      pi2,
      createExecuteIx({
        payer: ctx.payer.publicKey,
        walletPda,
        authorityPda: ownerAuthPda,
        vaultPda,
        packedInstructions: packed,
        authPayload: ap2,
        remainingAccounts: [
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
          { pubkey: execRecipient, isSigner: false, isWritable: true },
        ],
      programId: PROGRAM_ID,
      }),
    ]);

    // Verify counter = 2
    auth = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      ownerAuthPda,
    );
    expect(Number(auth.counter)).toBe(2);
  });

  it('two Secp256r1 authorities have independent counters', async () => {
    const key1 = await generateMockSecp256r1Key();
    const key2 = await generateMockSecp256r1Key();
    const userSeed = crypto.randomBytes(32);

    const [walletPda] = findWalletPda(userSeed, PROGRAM_ID);
    const [vaultPda] = findVaultPda(walletPda, PROGRAM_ID);
    const [auth1Pda, auth1Bump] = findAuthorityPda(
      walletPda,
      key1.credentialIdHash,
      PROGRAM_ID,
    );

    // Create wallet with key1 as owner
    await sendTx(ctx, [
      createCreateWalletIx({
        payer: ctx.payer.publicKey,
        walletPda,
        vaultPda,
        authorityPda: auth1Pda,
        userSeed,
        authType: AUTH_TYPE_SECP256R1,
        authBump: auth1Bump,
        credentialOrPubkey: key1.credentialIdHash,
        secp256r1Pubkey: key1.publicKeyBytes,
        rpId: key1.rpId,
      programId: PROGRAM_ID,
      }),
    ]);

    // Add key2 as spender via key1 (counter1 goes to 1)
    const [auth2Pda] = findAuthorityPda(walletPda, key2.credentialIdHash, PROGRAM_ID);
    const slot = await getSlot(ctx);

    const rpIdBytes = Buffer.from(key2.rpId, 'utf-8');
    const dataPayload2 = Buffer.concat([
      Buffer.from([AUTH_TYPE_SECP256R1, ROLE_SPENDER]),
      Buffer.alloc(6),
      key2.credentialIdHash,
      key2.publicKeyBytes,
      Buffer.from([rpIdBytes.length]),
      rpIdBytes,
    ]);
    // On-chain extends: extended_data_payload = data_payload + payer.key()
    const signedPayloadAdd = Buffer.concat([
      dataPayload2,
      ctx.payer.publicKey.toBuffer(),
    ]);

    const { authPayload, precompileIx } = await signSecp256r1({
      key: key1,
      discriminator: new Uint8Array([DISC_ADD_AUTHORITY]),
      signedPayload: signedPayloadAdd,
      slot,
      counter: 1,
      payer: ctx.payer.publicKey,
      sysvarIxIndex: 6,
    });

    await sendTx(ctx, [
      precompileIx,
      createAddAuthorityIx({
        payer: ctx.payer.publicKey,
        walletPda,
        adminAuthorityPda: auth1Pda,
        newAuthorityPda: auth2Pda,
        newType: AUTH_TYPE_SECP256R1,
        newRole: ROLE_SPENDER,
        credentialOrPubkey: key2.credentialIdHash,
        secp256r1Pubkey: key2.publicKeyBytes,
        rpId: key2.rpId,
        authPayload,
      programId: PROGRAM_ID,
      }),
    ]);

    // Verify: auth1 counter=1, auth2 counter=0
    const a1 = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      auth1Pda,
    );
    const a2 = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      auth2Pda,
    );
    expect(Number(a1.counter)).toBe(1);
    expect(Number(a2.counter)).toBe(0);
  });
});
