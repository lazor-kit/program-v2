import * as anchor from '@coral-xyz/anchor';
import ECDSA from 'ecdsa-secp256r1';
import { expect } from 'chai';
import { sendAndConfirmTransaction, Transaction } from '@solana/web3.js';
import * as dotenv from 'dotenv';
import { base64, bs58 } from '@coral-xyz/anchor/dist/cjs/utils/bytes';
import { LazorkitClient } from '../contract-integration';
dotenv.config();

describe('Test smart wallet with default policy', () => {
  const connection = new anchor.web3.Connection(
    process.env.RPC_URL || 'http://localhost:8899',
    'confirmed'
  );

  const lazorkitProgram = new LazorkitClient(connection);

  const payer = anchor.web3.Keypair.fromSecretKey(
    bs58.decode(process.env.PRIVATE_KEY!)
  );

  before(async () => {
    // airdrop some SOL to the payer

    const programConfig = await connection.getAccountInfo(
      lazorkitProgram.configPda()
    );

    if (programConfig === null) {
      const ix = await lazorkitProgram.buildInitializeInstruction(
        payer.publicKey
      );
      const txn = new Transaction().add(ix);

      const sig = await sendAndConfirmTransaction(connection, txn, [payer], {
        commitment: 'confirmed',
        skipPreflight: true,
      });

      console.log('Initialize txn: ', sig);
    }
  });

  it('Init smart wallet with default policy successfully', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();
    const smartWallet = lazorkitProgram.smartWalletPda(smartWalletId);

    const walletDevice = lazorkitProgram.walletDevicePda(
      smartWallet,
      passkeyPubkey
    );

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTransaction({
        payer: payer.publicKey,
        passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        isPayForUser: true,
        smartWalletId,
      });

    const sig = await sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn,
      [payer],
      {
        commitment: 'confirmed',
      }
    );

    console.log('Create smart-wallet: ', sig);

    const smartWalletData = await lazorkitProgram.getSmartWalletData(
      smartWallet
    );

    expect(smartWalletData.id.toString()).to.be.equal(smartWalletId.toString());

    const walletDeviceData = await lazorkitProgram.getWalletDeviceData(
      walletDevice
    );

    expect(walletDeviceData.passkeyPubkey.toString()).to.be.equal(
      passkeyPubkey.toString()
    );
    expect(walletDeviceData.smartWallet.toString()).to.be.equal(
      smartWallet.toString()
    );
  });

  xit('Create address lookup table', async () => {
    const slot = await connection.getSlot();

    const [lookupTableInst, lookupTableAddress] =
      anchor.web3.AddressLookupTableProgram.createLookupTable({
        authority: payer.publicKey,
        payer: payer.publicKey,
        recentSlot: slot,
      });

    const txn = new anchor.web3.Transaction().add(lookupTableInst);

    await sendAndConfirmTransaction(connection, txn, [payer], {
      commitment: 'confirmed',
      skipPreflight: true,
    });

    console.log('Lookup table: ', lookupTableAddress);

    const extendInstruction =
      anchor.web3.AddressLookupTableProgram.extendLookupTable({
        payer: payer.publicKey,
        authority: payer.publicKey,
        lookupTable: lookupTableAddress,
        addresses: [
          lazorkitProgram.configPda(),
          lazorkitProgram.policyProgramRegistryPda(),
          lazorkitProgram.defaultPolicyProgram.programId,
          anchor.web3.SystemProgram.programId,
          anchor.web3.SYSVAR_RENT_PUBKEY,
          anchor.web3.SYSVAR_CLOCK_PUBKEY,
          anchor.web3.SYSVAR_RENT_PUBKEY,
          anchor.web3.SYSVAR_RENT_PUBKEY,
          lazorkitProgram.programId,
        ],
      });

    const txn1 = new anchor.web3.Transaction().add(extendInstruction);

    const sig1 = await sendAndConfirmTransaction(connection, txn1, [payer], {
      commitment: 'confirmed',
    });

    console.log('Extend lookup table: ', sig1);
  });
});
