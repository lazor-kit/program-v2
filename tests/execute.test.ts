import * as anchor from '@coral-xyz/anchor';
import ECDSA from 'ecdsa-secp256r1';
import { expect } from 'chai';
import * as dotenv from 'dotenv';
import { base64, bs58 } from '@coral-xyz/anchor/dist/cjs/utils/bytes';
import {
  buildCreateChunkMessage,
  buildExecuteMessage,
  DefaultPolicyClient,
  LazorkitClient,
} from '../contract-integration';
import { createTransferInstruction } from '@solana/spl-token';
import { buildFakeMessagePasskey, createNewMint, mintTokenTo } from './utils';
dotenv.config();

describe('Test smart wallet with default policy', () => {
  const connection = new anchor.web3.Connection(
    process.env.RPC_URL || 'http://localhost:8899',
    'confirmed'
  );

  const lazorkitProgram = new LazorkitClient(connection);
  const defaultPolicyClient = new DefaultPolicyClient(connection);

  const payer = anchor.web3.Keypair.fromSecretKey(
    bs58.decode(process.env.PRIVATE_KEY!)
  );

  before(async () => {
    // airdrop some SOL to the payer

    const config = await connection.getAccountInfo(
      lazorkitProgram.getConfigPubkey()
    );

    if (config === null) {
      const ix = await lazorkitProgram.buildInitializeProgramIns(
        payer.publicKey
      );
      const txn = new anchor.web3.Transaction().add(ix);

      const sig = await anchor.web3.sendAndConfirmTransaction(connection, txn, [
        payer,
      ]);

      console.log('Initialize txn: ', sig);
    }
  });

  xit('Init smart wallet with default policy successfully', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const walletDevice = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      passkeyPubkey
    );

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        smartWalletId,
        amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
      });

    const sig = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer],
      {
        commitment: 'confirmed',
      }
    );

    console.log('Create smart-wallet: ', sig);

    const smartWalletConfig = await lazorkitProgram.getSmartWalletConfigData(
      smartWallet
    );

    expect(smartWalletConfig.walletId.toString()).to.be.equal(
      smartWalletId.toString()
    );

    const walletDeviceData = await lazorkitProgram.getWalletDeviceData(
      walletDevice
    );

    expect(walletDeviceData.passkeyPublicKey.toString()).to.be.equal(
      passkeyPubkey.toString()
    );
    expect(walletDeviceData.smartWalletAddress.toString()).to.be.equal(
      smartWallet.toString()
    );
  });

  xit('Execute direct transaction with transfer sol from smart wallet', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const walletDevice = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      passkeyPubkey
    );

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        smartWalletId,
        amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
      });

    await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    const transferFromSmartWalletIns = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: anchor.web3.Keypair.generate().publicKey,
      lamports: 0.001 * anchor.web3.LAMPORTS_PER_SOL,
    });

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      walletDevice,
      smartWallet
    );

    const timestamp = new anchor.BN(Math.floor(Date.now() / 1000));

    const plainMessage = buildExecuteMessage(
      payer.publicKey,
      smartWallet,
      new anchor.BN(0),
      timestamp,
      checkPolicyIns,
      transferFromSmartWalletIns
    );

    const { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
      await buildFakeMessagePasskey(plainMessage);

    const signature = privateKey.sign(message);

    const executeDirectTransactionTxn = await lazorkitProgram.executeTxn({
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeySignature: {
        passkeyPublicKey: passkeyPubkey,
        signature64: signature,
        clientDataJsonRaw64: clientDataJsonRaw64,
        authenticatorDataRaw64: authenticatorDataRaw64,
      },
      policyInstruction: checkPolicyIns,
      cpiInstruction: transferFromSmartWalletIns,
      vaultIndex: 0,
      timestamp,
    });

    const sig2 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      executeDirectTransactionTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Execute direct transaction: ', sig2);
  });

  it('Execute chunk transaction with transfer token from smart wallet', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const walletDevice = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      passkeyPubkey
    );

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        smartWalletId,
        amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
      });

    const sig1 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Create smart wallet: ', sig1);

    // create mint
    const mint = await createNewMint(connection, payer, 6);

    // create token account
    const payerTokenAccount = await mintTokenTo(
      connection,
      mint,
      payer,
      payer,
      payer.publicKey,
      10 * 10 ** 6
    );

    const smartWalletTokenAccount = await mintTokenTo(
      connection,
      mint,
      payer,
      payer,
      smartWallet,
      100 * 10 ** 6
    );

    const transferTokenIns = createTransferInstruction(
      smartWalletTokenAccount,
      payerTokenAccount,
      smartWallet,
      10 * 10 ** 6
    );

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      walletDevice,
      smartWallet
    );

    const timestamp = new anchor.BN(Math.floor(Date.now() / 1000));

    const plainMessage = buildCreateChunkMessage(
      payer.publicKey,
      smartWallet,
      new anchor.BN(0),
      timestamp,
      checkPolicyIns,
      [transferTokenIns]
    );

    const { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
      await buildFakeMessagePasskey(plainMessage);

    const signature = privateKey.sign(message);

    const createDeferredExecutionTxn = await lazorkitProgram.createChunkTxn({
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeySignature: {
        passkeyPublicKey: passkeyPubkey,
        signature64: signature,
        clientDataJsonRaw64: clientDataJsonRaw64,
        authenticatorDataRaw64: authenticatorDataRaw64,
      },
      policyInstruction: null,
      cpiInstructions: [transferTokenIns],
      timestamp,
      vaultIndex: 0,
    });

    const sig2 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createDeferredExecutionTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Create deferred execution: ', sig2);

    const executeDeferredTransactionTxn =
      (await lazorkitProgram.executeChunkTxn(
        {
          payer: payer.publicKey,
          smartWallet: smartWallet,
          cpiInstructions: [transferTokenIns],
        },
        {
          useVersionedTransaction: true,
        }
      )) as anchor.web3.VersionedTransaction;

    executeDeferredTransactionTxn.sign([payer]);
    const sig3 = await connection.sendTransaction(
      executeDeferredTransactionTxn
    );
    await connection.confirmTransaction(sig3);

    console.log('Execute deferred transaction: ', sig3);

    //
    const getSmartWalletByPasskey =
      await lazorkitProgram.getSmartWalletByPasskey(passkeyPubkey);

    console.log('Get smart wallet by passkey: ', getSmartWalletByPasskey);

    expect(getSmartWalletByPasskey.smartWallet?.toString()).to.be.equal(
      smartWallet.toString()
    );
  });

  xit('Execute deferred transaction with transfer token from smart wallet and transfer sol from smart_wallet', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const walletDevice = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      passkeyPubkey
    );

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        smartWalletId,
        amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
      });

    const sig1 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Create smart wallet: ', sig1);

    // create mint
    const mint = await createNewMint(connection, payer, 6);

    // create token account
    const payerTokenAccount = await mintTokenTo(
      connection,
      mint,
      payer,
      payer,
      payer.publicKey,
      10 * 10 ** 6
    );

    const smartWalletTokenAccount = await mintTokenTo(
      connection,
      mint,
      payer,
      payer,
      smartWallet,
      100 * 10 ** 6
    );

    const transferTokenIns = createTransferInstruction(
      smartWalletTokenAccount,
      payerTokenAccount,
      smartWallet,
      10 * 10 ** 6
    );

    const transferFromSmartWalletIns = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: anchor.web3.Keypair.generate().publicKey,
      lamports: 0.01 * anchor.web3.LAMPORTS_PER_SOL,
    });

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      walletDevice,
      smartWallet
    );

    const timestamp = new anchor.BN(Math.floor(Date.now() / 1000));

    const plainMessage = buildCreateChunkMessage(
      payer.publicKey,
      smartWallet,
      new anchor.BN(0),
      timestamp,
      checkPolicyIns,
      [transferTokenIns, transferFromSmartWalletIns]
    );

    const { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
      await buildFakeMessagePasskey(plainMessage);

    const signature = privateKey.sign(message);

    const createDeferredExecutionTxn = await lazorkitProgram.createChunkTxn({
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeySignature: {
        passkeyPublicKey: passkeyPubkey,
        signature64: signature,
        clientDataJsonRaw64: clientDataJsonRaw64,
        authenticatorDataRaw64: authenticatorDataRaw64,
      },
      policyInstruction: null,
      cpiInstructions: [transferTokenIns, transferFromSmartWalletIns],
      vaultIndex: 0,
      timestamp,
    });

    const sig2 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createDeferredExecutionTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Create deferred execution: ', sig2);

    const executeDeferredTransactionTxn =
      (await lazorkitProgram.executeChunkTxn(
        {
          payer: payer.publicKey,
          smartWallet: smartWallet,
          cpiInstructions: [transferTokenIns, transferFromSmartWalletIns],
        },
        {
          useVersionedTransaction: true,
        }
      )) as anchor.web3.VersionedTransaction;

    executeDeferredTransactionTxn.sign([payer]);
    const sig3 = await connection.sendTransaction(
      executeDeferredTransactionTxn
    );
    await connection.confirmTransaction(sig3);

    console.log('Execute deferred transaction: ', sig3);
  });

  xit('Execute deferred transaction with transfer token from smart wallet and transfer sol from smart_wallet', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const walletDevice = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      passkeyPubkey
    );

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        smartWalletId,
        amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
      });

    const sig1 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Create smart wallet: ', sig1);

    // create mint
    const mint = await createNewMint(connection, payer, 6);

    // create token account
    const payerTokenAccount = await mintTokenTo(
      connection,
      mint,
      payer,
      payer,
      payer.publicKey,
      10 * 10 ** 6
    );

    const smartWalletTokenAccount = await mintTokenTo(
      connection,
      mint,
      payer,
      payer,
      smartWallet,
      100 * 10 ** 6
    );

    const transferTokenIns = createTransferInstruction(
      smartWalletTokenAccount,
      payerTokenAccount,
      smartWallet,
      10 * 10 ** 6
    );

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      walletDevice,
      smartWallet
    );

    const timestamp = new anchor.BN(Math.floor(Date.now() / 1000));

    const plainMessage = buildCreateChunkMessage(
      payer.publicKey,
      smartWallet,
      new anchor.BN(0),
      timestamp,
      checkPolicyIns,
      [transferTokenIns, transferTokenIns]
    );

    const { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
      await buildFakeMessagePasskey(plainMessage);

    const signature = privateKey.sign(message);

    const createDeferredExecutionTxn = await lazorkitProgram.createChunkTxn({
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeySignature: {
        passkeyPublicKey: passkeyPubkey,
        signature64: signature,
        clientDataJsonRaw64: clientDataJsonRaw64,
        authenticatorDataRaw64: authenticatorDataRaw64,
      },
      policyInstruction: null,
      cpiInstructions: [transferTokenIns, transferTokenIns],
      timestamp,
      vaultIndex: 0,
    });

    const sig2 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createDeferredExecutionTxn as anchor.web3.Transaction,
      [payer]
    );

    const executeDeferredTransactionTxn =
      (await lazorkitProgram.executeChunkTxn(
        {
          payer: payer.publicKey,
          smartWallet: smartWallet,
          cpiInstructions: [transferTokenIns, transferTokenIns],
        },
        {
          useVersionedTransaction: true,
        }
      )) as anchor.web3.VersionedTransaction;

    executeDeferredTransactionTxn.sign([payer]);
    const sig3 = await connection.sendTransaction(
      executeDeferredTransactionTxn
    );
    await connection.confirmTransaction(sig3);

    // log execute deferred transaction size
    const executeDeferredTransactionSize =
      executeDeferredTransactionTxn.serialize().length;

    console.log('Execute deferred transaction: ', sig3);
  });
});
