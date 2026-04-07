import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import { describe, it, expect, beforeAll } from 'vitest';
import {
  findVaultPda,
  findAuthorityPda,
  AuthType,
  Role,
  AuthorityAccount,
} from '@lazorkit/solita-client';
import { setupTest, sendTx, TestContext } from './common';
import { generateMockSecp256r1Signer } from './secp256r1Utils';

function getRandomSeed() {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  return seed;
}

describe('Counter increases for all Secp256r1 authority instructions', () => {
  let ctx: TestContext;
  let walletPda: PublicKey;
  let secpAdmin: any;
  let secpAdminAuthPda: PublicKey;
  let owner: Keypair;

  beforeAll(async () => {
    ctx = await setupTest();
    owner = Keypair.generate();
    const userSeed = getRandomSeed();
    // Create wallet
    const { ix, walletPda: w } = await ctx.highClient.createWallet({
      payer: ctx.payer,
      authType: AuthType.Ed25519,
      owner: owner.publicKey,
      userSeed,
    });
    await sendTx(ctx, [ix]);
    walletPda = w;
    // Generate Secp256r1 signer
    secpAdmin = await generateMockSecp256r1Signer();
    // Add Secp256r1 Admin
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
    // derive admin PDA
    const [aPda] = findAuthorityPda(walletPda, secpAdmin.credentialIdHash);
    secpAdminAuthPda = aPda;
  }, 30_000);

  it('counter increases for AddAuthority', async () => {
    const signer2 = await generateMockSecp256r1Signer();
    const { precompileIx, addIx } =
      await ctx.highClient.addAuthorityWithSecp256r1({
        payer: ctx.payer,
        walletPda,
        signer: secpAdmin,
        newAuthType: AuthType.Secp256r1,
        newAuthPubkey: signer2.publicKeyBytes,
        newCredentialHash: signer2.credentialIdHash,
        role: Role.Spender,
      });
    try {
      await sendTx(ctx, [precompileIx, addIx]);
    } catch (e: any) {
      console.error('AddAuthority tx failed:', e?.logs ?? e?.message ?? e);
      throw e;
    }
    const authAfter = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      secpAdminAuthPda,
    );
    expect(Number(authAfter.counter)).toBe(1);
  });

  it('counter increases for RemoveAuthority', async () => {
    // Use AddAuthority to add then Remove it
    const signer3 = await generateMockSecp256r1Signer();
    const { precompileIx: addPIx, addIx } =
      await ctx.highClient.addAuthorityWithSecp256r1({
        payer: ctx.payer,
        walletPda,
        signer: secpAdmin,
        newAuthType: AuthType.Secp256r1,
        newAuthPubkey: signer3.publicKeyBytes,
        newCredentialHash: signer3.credentialIdHash,
        role: Role.Spender,
      });
    await sendTx(ctx, [addPIx, addIx]);
    // Remove: derive authority PDA for the Secp credential hash and remove it
    const [toRemovePda] = findAuthorityPda(walletPda, signer3.credentialIdHash);
    const { precompileIx, removeIx } =
      await ctx.highClient.removeAuthorityWithSecp256r1({
        payer: ctx.payer,
        walletPda,
        signer: secpAdmin,
        authorityToRemovePda: toRemovePda,
      });
    try {
      await sendTx(ctx, [precompileIx, removeIx]);
    } catch (e: any) {
      console.error('RemoveAuthority tx failed:', e?.logs ?? e?.message ?? e);
      throw e;
    }
    const authAfter = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      secpAdminAuthPda,
    );
    expect(Number(authAfter.counter)).toBe(3);
  });

  it('counter increases for TransferOwnership', async () => {
    // Create a fresh Secp owner and transfer ownership to it using Ed25519 owner,
    // then have that Secp owner perform a Secp transfer to another Secp key. We assert
    // the original `secpAdmin` counter remains unchanged (should be 3 from previous ops).
    const signer4 = await generateMockSecp256r1Signer();
    const signer5 = await generateMockSecp256r1Signer();

    const [ownerAuthorityPda] = findAuthorityPda(
      walletPda,
      owner.publicKey.toBytes(),
    );
    const [newOwnerPda] = findAuthorityPda(walletPda, signer4.credentialIdHash);

    // Transfer ownership from Ed25519 owner -> signer4 (Secp)
    const transferToSigner4 = await ctx.highClient.transferOwnership({
      payer: ctx.payer,
      walletPda,
      currentOwnerAuthority: ownerAuthorityPda,
      newOwnerAuthority: newOwnerPda,
      newAuthType: AuthType.Secp256r1,
      newAuthPubkey: signer4.publicKeyBytes,
      newCredentialHash: signer4.credentialIdHash,
      signer: owner,
    });
    await sendTx(ctx, [transferToSigner4], [owner]);

    // Now signer4 (Secp owner) transfers ownership to signer5 using Secp flow
    const { precompileIx, transferIx } =
      await ctx.highClient.transferOwnershipWithSecp256r1({
        payer: ctx.payer,
        walletPda,
        signer: signer4,
        newAuthPubkey: signer5.publicKeyBytes,
        newCredentialHash: signer5.credentialIdHash,
        newAuthType: AuthType.Secp256r1,
      });
    try {
      await sendTx(ctx, [precompileIx, transferIx]);
    } catch (e: any) {
      console.error('TransferOwnership tx failed:', e?.logs ?? e?.message ?? e);
      throw e;
    }

    const authAfter = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      secpAdminAuthPda,
    );
    expect(Number(authAfter.counter)).toBe(3);
  });

  it('counter increases for CreateSession', async () => {
    const sessionKey = Keypair.generate().publicKey;
    const expiresAt = Math.floor(Date.now() / 1000) + 1000;
    const { precompileIx, sessionIx } =
      await ctx.highClient.createSessionWithSecp256r1({
        payer: ctx.payer,
        walletPda,
        signer: secpAdmin,
        sessionKey,
        expiresAt,
      });
    try {
      await sendTx(ctx, [precompileIx, sessionIx]);
    } catch (e: any) {
      console.error('CreateSession tx failed:', e?.logs ?? e?.message ?? e);
      throw e;
    }
    const authAfter = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      secpAdminAuthPda,
    );
    expect(Number(authAfter.counter)).toBe(4);
  });

  it('counter multi-increment: loop 10 executions', async () => {
    let before = await AuthorityAccount.fromAccountAddress(
      ctx.connection,
      secpAdminAuthPda,
    );
    const vaultPda = findVaultPda(walletPda)[0];

    // Ensure vault has sufficient funds for inner transfers used in the loop
    const fundIx = SystemProgram.transfer({
      fromPubkey: ctx.payer.publicKey,
      toPubkey: vaultPda,
      lamports: 2 * LAMPORTS_PER_SOL,
    });
    await sendTx(ctx, [fundIx]);

    for (let i = 1; i <= 10; i++) {
      const recipient = Keypair.generate();
      const transferIx = SystemProgram.transfer({
        fromPubkey: vaultPda,
        toPubkey: recipient.publicKey,
        lamports: 0.01 * LAMPORTS_PER_SOL,
      });
      const { precompileIx, executeIx } =
        await ctx.highClient.executeWithSecp256r1({
          payer: ctx.payer,
          walletPda,
          signer: secpAdmin,
          innerInstructions: [transferIx],
        });
      await sendTx(ctx, [precompileIx, executeIx]);
      const after = await AuthorityAccount.fromAccountAddress(
        ctx.connection,
        secpAdminAuthPda,
      );
      expect(Number(after.counter)).toBe(Number(before.counter) + 1);
      before = after;
    }
  });
});
