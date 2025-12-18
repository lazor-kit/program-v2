import * as anchor from '@coral-xyz/anchor';
import ECDSA from 'ecdsa-secp256r1';
import { expect } from 'chai';
import * as dotenv from 'dotenv';
import { base64, bs58 } from '@coral-xyz/anchor/dist/cjs/utils/bytes';
import {
  DefaultPolicyClient,
  LazorkitClient,
  SmartWalletAction,
  asPasskeyPublicKey,
  asCredentialHash,
} from '../sdk';
import { createTransferInstruction } from '@solana/spl-token';
import { buildFakeMessagePasskey, createNewMint, mintTokenTo } from './utils';
dotenv.config();

// Helper function to get blockchain timestamp
async function getBlockchainTimestamp(
  connection: anchor.web3.Connection
): Promise<anchor.BN> {
  const slot = await connection.getSlot();
  const blockTime = await connection.getBlockTime(slot);
  return new anchor.BN(blockTime || Math.floor(Date.now() / 1000));
}

describe('Test smart wallet with default policy', () => {
  const connection = new anchor.web3.Connection(
    process.env.CLUSTER != 'localhost'
      ? process.env.RPC_URL
      : 'http://localhost:8899',
    'confirmed'
  );

  const lazorkitProgram = new LazorkitClient(connection);
  const defaultPolicyClient = new DefaultPolicyClient(connection);

  const payer = anchor.web3.Keypair.fromSecretKey(
    bs58.decode(process.env.PRIVATE_KEY)
  );

  it('Init smart wallet with default policy successfully', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = asPasskeyPublicKey(
      Array.from(Buffer.from(publicKeyBase64, 'base64'))
    );

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        smartWalletId,
      });

    const sig = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Create smart-wallet: ', sig);

    const smartWalletConfig = await lazorkitProgram.getWalletStateData(
      smartWallet
    );

    expect(smartWalletConfig.walletId.toString()).to.be.equal(
      smartWalletId.toString()
    );
  });

  it('Execute direct transaction with transfer sol from smart wallet', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = asPasskeyPublicKey(
      Array.from(Buffer.from(publicKeyBase64, 'base64'))
    );

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const credentialHash = asCredentialHash(
      Array.from(
        new Uint8Array(
          require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
        )
      )
    );

    const policySigner = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      credentialHash
    );

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        smartWalletId,
        amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
      });

    await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    const walletStateData = await lazorkitProgram.getWalletStateData(
      smartWallet
    );

    const transferFromSmartWalletIns = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: payer.publicKey,
      lamports: 0.001 * anchor.web3.LAMPORTS_PER_SOL,
    });

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx({
      walletId: smartWalletId,
      passkeyPublicKey: passkeyPubkey,
      policySigner,
      smartWallet,
      credentialHash: asCredentialHash(credentialHash),
      policyData: walletStateData.policyData,
    });

    const timestamp = await getBlockchainTimestamp(connection);

    const plainMessage = await lazorkitProgram.buildAuthorizationMessage({
      action: {
        type: SmartWalletAction.Execute,
        args: {
          policyInstruction: checkPolicyIns,
          cpiInstruction: transferFromSmartWalletIns,
        },
      },
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeyPublicKey: passkeyPubkey,
      credentialHash: credentialHash,
      timestamp: new anchor.BN(timestamp),
    });

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
      timestamp,
      credentialHash,
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

    const passkeyPubkey = asPasskeyPublicKey(
      Array.from(Buffer.from(publicKeyBase64, 'base64'))
    );

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const credentialHash = asCredentialHash(
      Array.from(
        new Uint8Array(
          require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
        )
      )
    );

    const policySigner = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      credentialHash
    );

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        smartWalletId,
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

    const walletStateData = await lazorkitProgram.getWalletStateData(
      smartWallet
    );

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx({
      walletId: smartWalletId,
      passkeyPublicKey: passkeyPubkey,
      policySigner,
      smartWallet,
      credentialHash: asCredentialHash(credentialHash),
      policyData: walletStateData.policyData,
    });

    const timestamp = await getBlockchainTimestamp(connection);

    const plainMessage = await lazorkitProgram.buildAuthorizationMessage({
      action: {
        type: SmartWalletAction.CreateChunk,
        args: {
          policyInstruction: checkPolicyIns,
          cpiInstructions: [transferTokenIns],
        },
      },
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeyPublicKey: passkeyPubkey,
      credentialHash: credentialHash,
      timestamp: new anchor.BN(timestamp),
    });

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
      credentialHash,
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
          computeUnitLimit: 300_000,
        }
      )) as anchor.web3.Transaction;

    executeDeferredTransactionTxn.sign(payer);
    const sig3 = await connection.sendRawTransaction(
      executeDeferredTransactionTxn.serialize()
    );
    await connection.confirmTransaction(sig3);

    console.log('Execute deferred transaction: ', sig3);
  });

  it('Execute deferred transaction with multiple CPI instructions', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = asPasskeyPublicKey(
      Array.from(Buffer.from(publicKeyBase64, 'base64'))
    );

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const credentialHash = asCredentialHash(
      Array.from(
        new Uint8Array(
          require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
        )
      )
    );

    const policySigner = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      credentialHash
    );

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        smartWalletId,
        amount: new anchor.BN(0.1 * anchor.web3.LAMPORTS_PER_SOL),
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
      toPubkey: payer.publicKey,
      lamports: 0.01 * anchor.web3.LAMPORTS_PER_SOL,
    });

    const walletStateData = await lazorkitProgram.getWalletStateData(
      smartWallet
    );

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx({
      walletId: smartWalletId,
      passkeyPublicKey: passkeyPubkey,
      policySigner,
      smartWallet,
      credentialHash: asCredentialHash(credentialHash),
      policyData: walletStateData.policyData,
    });

    const timestamp = await getBlockchainTimestamp(connection);

    const cpiInstructions = [
      transferTokenIns,
      transferFromSmartWalletIns,
      transferFromSmartWalletIns,
      transferTokenIns,
      transferTokenIns,
    ];

    const plainMessage = await lazorkitProgram.buildAuthorizationMessage({
      action: {
        type: SmartWalletAction.CreateChunk,
        args: {
          policyInstruction: checkPolicyIns,
          cpiInstructions,
        },
      },
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeyPublicKey: passkeyPubkey,
      credentialHash: credentialHash,
      timestamp: new anchor.BN(timestamp),
    });

    const { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
      await buildFakeMessagePasskey(plainMessage);

    const signature = privateKey.sign(message);

    const createDeferredExecutionTxn = await lazorkitProgram.createChunkTxn(
      {
        payer: payer.publicKey,
        smartWallet: smartWallet,

        passkeySignature: {
          passkeyPublicKey: passkeyPubkey,
          signature64: signature,
          clientDataJsonRaw64: clientDataJsonRaw64,
          authenticatorDataRaw64: authenticatorDataRaw64,
        },
        policyInstruction: null,
        cpiInstructions,

        timestamp,
        credentialHash,
      },
      {
        computeUnitLimit: 300_000,
      }
    );

    const sig2 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createDeferredExecutionTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Create deferred execution: ', sig2);

    const executeDeferredTransactionTxn =
      (await lazorkitProgram.executeChunkTxn({
        payer: payer.publicKey,
        smartWallet: smartWallet,
        cpiInstructions,
      })) as anchor.web3.Transaction;

    executeDeferredTransactionTxn.sign(payer);
    const sig3 = await connection.sendRawTransaction(
      executeDeferredTransactionTxn.serialize(),
      {
        skipPreflight: true,
      }
    );

    console.log('Execute deferred transaction: ', sig3);
  });
});
