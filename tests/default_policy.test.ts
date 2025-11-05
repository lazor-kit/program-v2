import * as anchor from '@coral-xyz/anchor';
import ECDSA from 'ecdsa-secp256r1';
import { expect } from 'chai';
import * as dotenv from 'dotenv';
import { base64, bs58 } from '@coral-xyz/anchor/dist/cjs/utils/bytes';
import {
  DefaultPolicyClient,
  LazorkitClient,
  SmartWalletAction,
} from '../contract-integration';
import { buildFakeMessagePasskey } from './utils';
dotenv.config();

// Helper function to get real blockchain timestamp
async function getBlockchainTimestamp(
  connection: anchor.web3.Connection
): Promise<anchor.BN> {
  const slot = await connection.getSlot();
  const timestamp = await connection.getBlockTime(slot);
  return new anchor.BN(timestamp || Math.floor(Date.now() / 1000));
}

describe.skip('Test smart wallet with default policy', () => {
  const connection = new anchor.web3.Connection(
    process.env.CLUSTER != 'localhost'
      ? process.env.RPC_URL
      : 'http://localhost:8899',
    'confirmed'
  );

  const lazorkitProgram = new LazorkitClient(connection);
  const defaultPolicyClient = new DefaultPolicyClient(connection);

  const payer = anchor.web3.Keypair.fromSecretKey(
    bs58.decode(process.env.PRIVATE_KEY!)
  );

  it('Add one device to smart wallet', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const credentialHash = Array.from(
      new Uint8Array(
        require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
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
        policyInstruction: null,
        smartWalletId,
        amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
      });

    const createSmartWalletSig = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Create smart wallet txn: ', createSmartWalletSig);

    const privateKey2 = ECDSA.generateKey();

    const publicKeyBase642 = privateKey2.toCompressedPublicKey();

    const passkeyPubkey2 = Array.from(Buffer.from(publicKeyBase642, 'base64'));
    const walletStateData = await lazorkitProgram.getWalletStateData(
      smartWallet
    );

    const credentialId2 = base64.encode(Buffer.from('testing2')); // random string
    const credentialHash2 = Array.from(
      new Uint8Array(
        require('js-sha256').arrayBuffer(Buffer.from(credentialId2, 'base64'))
      )
    );

    const addDeviceIx = await defaultPolicyClient.buildAddDeviceIx(
      smartWalletId,
      passkeyPubkey,
      credentialHash,
      walletStateData.policyData,
      passkeyPubkey2,
      credentialHash2,
      smartWallet,
      policySigner
    );

    const timestamp = await getBlockchainTimestamp(connection);

    const plainMessage = await lazorkitProgram.buildAuthorizationMessage({
      action: {
        type: SmartWalletAction.AddDevice,
        args: {
          policyInstruction: addDeviceIx,
          newDevicePasskeyPublicKey: passkeyPubkey2,
          newDeviceCredentialHash: credentialHash2,
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

    const callPolicyTxn = await lazorkitProgram.addDeviceTxn({
      payer: payer.publicKey,
      smartWallet,
      passkeySignature: {
        passkeyPublicKey: passkeyPubkey,
        signature64: signature,
        clientDataJsonRaw64: clientDataJsonRaw64,
        authenticatorDataRaw64: authenticatorDataRaw64,
      },
      policyInstruction: addDeviceIx,
      newDevicePasskeyPublicKey: passkeyPubkey2,
      newDeviceCredentialHash: credentialHash2,
      timestamp,
      credentialHash: credentialHash,
    });

    const sig = await anchor.web3.sendAndConfirmTransaction(
      connection,
      callPolicyTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Add device txn: ', sig);
  });

  // xit('Add 2 devices to smart wallet', async () => {
  //   // Create initial smart wallet with first device
  //   const privateKey1 = ECDSA.generateKey();
  //   const publicKeyBase64_1 = privateKey1.toCompressedPublicKey();
  //   const passkeyPubkey1 = Array.from(Buffer.from(publicKeyBase64_1, 'base64'));

  //   const smartWalletId = lazorkitProgram.generateWalletId();
  //   const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);
  //   const credentialId = base64.encode(Buffer.from('testing'));

  //   const walletDevice1 = lazorkitProgram.getWalletDevicePubkey(
  //     smartWallet,
  //     passkeyPubkey1
  //   );

  //   // Create smart wallet
  //   const { transaction: createSmartWalletTxn } =
  //     await lazorkitProgram.createSmartWalletTxn({
  //       payer: payer.publicKey,
  //       passkeyPublicKey: passkeyPubkey1,
  //       credentialIdBase64: credentialId,
  //       policyInstruction: null,
  //       smartWalletId,
  //       amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
  //     });

  //   await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     createSmartWalletTxn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Created smart wallet with first device');

  //   // Generate 2 additional devices
  //   const privateKey2 = ECDSA.generateKey();
  //   const publicKeyBase64_2 = privateKey2.toCompressedPublicKey();
  //   const passkeyPubkey2 = Array.from(Buffer.from(publicKeyBase64_2, 'base64'));

  //   const privateKey3 = ECDSA.generateKey();
  //   const publicKeyBase64_3 = privateKey3.toCompressedPublicKey();
  //   const passkeyPubkey3 = Array.from(Buffer.from(publicKeyBase64_3, 'base64'));

  //   const walletDevice2 = lazorkitProgram.getWalletDevicePubkey(
  //     smartWallet,
  //     passkeyPubkey2
  //   );

  //   const walletDevice3 = lazorkitProgram.getWalletDevicePubkey(
  //     smartWallet,
  //     passkeyPubkey3
  //   );

  //   // Add second device
  //   const addDevice2Ix = await defaultPolicyClient.buildAddDeviceIx(
  //     smartWalletId,
  //     passkeyPubkey1,
  //     passkeyPubkey2,
  //     smartWallet,
  //     walletDevice1,
  //     walletDevice2
  //   );

  //   let timestamp = await getBlockchainTimestamp(connection);
  //   let nonce = await getLatestNonce(lazorkitProgram, smartWallet);
  //   let plainMessage = buildCallPolicyMessage(
  //     payer.publicKey,
  //     smartWallet,
  //     nonce,
  //     timestamp,
  //     addDevice2Ix
  //   );

  //   let { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
  //     await buildFakeMessagePasskey(plainMessage);

  //   let signature = privateKey1.sign(message);

  //   const addDevice2Txn = await lazorkitProgram.callPolicyTxn({
  //     payer: payer.publicKey,
  //     smartWallet,
  //     passkeySignature: {
  //       passkeyPublicKey: passkeyPubkey1,
  //       signature64: signature,
  //       clientDataJsonRaw64: clientDataJsonRaw64,
  //       authenticatorDataRaw64: authenticatorDataRaw64,
  //     },
  //     policyInstruction: addDevice2Ix,
  //     newWalletDevice: {
  //       passkeyPublicKey: passkeyPubkey2,
  //       credentialIdBase64: credentialId,
  //     },
  //     timestamp,
  //     vaultIndex: 0,
  //   });

  //   await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     addDevice2Txn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Added second device');

  //   // Add third device
  //   const addDevice3Ix = await defaultPolicyClient.buildAddDeviceIx(
  //     smartWalletId,
  //     passkeyPubkey1,
  //     passkeyPubkey3,
  //     smartWallet,
  //     walletDevice1,
  //     walletDevice3
  //   );

  //   timestamp = await getBlockchainTimestamp(connection);
  //   nonce = await getLatestNonce(lazorkitProgram, smartWallet);
  //   plainMessage = buildCallPolicyMessage(
  //     payer.publicKey,
  //     smartWallet,
  //     nonce,
  //     timestamp,
  //     addDevice3Ix
  //   );

  //   ({ message, clientDataJsonRaw64, authenticatorDataRaw64 } =
  //     await buildFakeMessagePasskey(plainMessage));

  //   signature = privateKey1.sign(message);

  //   const addDevice3Txn = await lazorkitProgram.callPolicyTxn({
  //     payer: payer.publicKey,
  //     smartWallet,
  //     passkeySignature: {
  //       passkeyPublicKey: passkeyPubkey1,
  //       signature64: signature,
  //       clientDataJsonRaw64: clientDataJsonRaw64,
  //       authenticatorDataRaw64: authenticatorDataRaw64,
  //     },
  //     policyInstruction: addDevice3Ix,
  //     newWalletDevice: {
  //       passkeyPublicKey: passkeyPubkey3,
  //       credentialIdBase64: credentialId,
  //     },
  //     timestamp,
  //     vaultIndex: 0,
  //   });

  //   await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     addDevice3Txn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Added third device');
  // });

  // xit('Add 1 device and remove it', async () => {
  //   // Create initial smart wallet with first device
  //   const privateKey1 = ECDSA.generateKey();
  //   const publicKeyBase64_1 = privateKey1.toCompressedPublicKey();
  //   const passkeyPubkey1 = Array.from(Buffer.from(publicKeyBase64_1, 'base64'));

  //   const smartWalletId = lazorkitProgram.generateWalletId();
  //   const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);
  //   const credentialId = base64.encode(Buffer.from('testing'));

  //   const walletDevice1 = lazorkitProgram.getWalletDevicePubkey(
  //     smartWallet,
  //     passkeyPubkey1
  //   );

  //   // Create smart wallet
  //   const { transaction: createSmartWalletTxn } =
  //     await lazorkitProgram.createSmartWalletTxn({
  //       payer: payer.publicKey,
  //       passkeyPublicKey: passkeyPubkey1,
  //       credentialIdBase64: credentialId,
  //       policyInstruction: null,
  //       smartWalletId,
  //       amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
  //     });

  //   await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     createSmartWalletTxn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Created smart wallet with first device');

  //   // Generate additional device
  //   const privateKey2 = ECDSA.generateKey();
  //   const publicKeyBase64_2 = privateKey2.toCompressedPublicKey();
  //   const passkeyPubkey2 = Array.from(Buffer.from(publicKeyBase64_2, 'base64'));

  //   const walletDevice2 = lazorkitProgram.getWalletDevicePubkey(
  //     smartWallet,
  //     passkeyPubkey2
  //   );

  //   // Add second device
  //   const addDevice2Ix = await defaultPolicyClient.buildAddDeviceIx(
  //     smartWalletId,
  //     passkeyPubkey1,
  //     passkeyPubkey2,
  //     smartWallet,
  //     walletDevice1,
  //     walletDevice2
  //   );

  //   let timestamp = await getBlockchainTimestamp(connection);
  //   let nonce = await getLatestNonce(lazorkitProgram, smartWallet);
  //   let plainMessage = buildCallPolicyMessage(
  //     payer.publicKey,
  //     smartWallet,
  //     nonce,
  //     timestamp,
  //     addDevice2Ix
  //   );

  //   let { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
  //     await buildFakeMessagePasskey(plainMessage);

  //   let signature = privateKey1.sign(message);

  //   const addDevice2Txn = await lazorkitProgram.callPolicyTxn({
  //     payer: payer.publicKey,
  //     smartWallet,
  //     passkeySignature: {
  //       passkeyPublicKey: passkeyPubkey1,
  //       signature64: signature,
  //       clientDataJsonRaw64: clientDataJsonRaw64,
  //       authenticatorDataRaw64: authenticatorDataRaw64,
  //     },
  //     policyInstruction: addDevice2Ix,
  //     newWalletDevice: {
  //       passkeyPublicKey: passkeyPubkey2,
  //       credentialIdBase64: credentialId,
  //     },
  //     timestamp,
  //     vaultIndex: 0,
  //   });

  //   await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     addDevice2Txn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Added second device');

  //   // Remove second device
  //   const removeDevice2Ix = await defaultPolicyClient.buildRemoveDeviceIx(
  //     smartWalletId,
  //     passkeyPubkey1,
  //     passkeyPubkey2,
  //     smartWallet,
  //     walletDevice1,
  //     walletDevice2
  //   );

  //   timestamp = await getBlockchainTimestamp(connection);
  //   nonce = await getLatestNonce(lazorkitProgram, smartWallet);
  //   plainMessage = buildCallPolicyMessage(
  //     payer.publicKey,
  //     smartWallet,
  //     nonce,
  //     timestamp,
  //     removeDevice2Ix
  //   );

  //   ({ message, clientDataJsonRaw64, authenticatorDataRaw64 } =
  //     await buildFakeMessagePasskey(plainMessage));

  //   signature = privateKey1.sign(message);

  //   const removeDevice2Txn = await lazorkitProgram.callPolicyTxn({
  //     payer: payer.publicKey,
  //     smartWallet,
  //     passkeySignature: {
  //       passkeyPublicKey: passkeyPubkey1,
  //       signature64: signature,
  //       clientDataJsonRaw64: clientDataJsonRaw64,
  //       authenticatorDataRaw64: authenticatorDataRaw64,
  //     },
  //     policyInstruction: removeDevice2Ix,
  //     timestamp,
  //     vaultIndex: 0,
  //   });

  //   await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     removeDevice2Txn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Removed second device');
  // });

  // it('Add 1 device and execute transaction with it', async () => {
  //   // Create initial smart wallet with first device
  //   const privateKey1 = ECDSA.generateKey();
  //   const publicKeyBase64_1 = privateKey1.toCompressedPublicKey();
  //   const passkeyPubkey1 = Array.from(Buffer.from(publicKeyBase64_1, 'base64'));

  //   const smartWalletId = lazorkitProgram.generateWalletId();
  //   const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);
  //   const credentialId = base64.encode(Buffer.from('testing'));

  //   const walletDevice1 = lazorkitProgram.getWalletDevicePubkey(
  //     smartWallet,
  //     passkeyPubkey1
  //   );

  //   // Create smart wallet
  //   const { transaction: createSmartWalletTxn } =
  //     await lazorkitProgram.createSmartWalletTxn({
  //       payer: payer.publicKey,
  //       passkeyPublicKey: passkeyPubkey1,
  //       credentialIdBase64: credentialId,
  //       policyInstruction: null,
  //       smartWalletId,
  //       amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
  //     });

  //   await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     createSmartWalletTxn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Created smart wallet with first device');

  //   // Generate additional device
  //   const privateKey2 = ECDSA.generateKey();
  //   const publicKeyBase64_2 = privateKey2.toCompressedPublicKey();
  //   const passkeyPubkey2 = Array.from(Buffer.from(publicKeyBase64_2, 'base64'));

  //   const walletDevice2 = lazorkitProgram.getWalletDevicePubkey(
  //     smartWallet,
  //     passkeyPubkey2
  //   );

  //   // Add second device
  //   const addDevice2Ix = await defaultPolicyClient.buildAddDeviceIx(
  //     smartWalletId,
  //     passkeyPubkey1,
  //     passkeyPubkey2,
  //     smartWallet,
  //     walletDevice1,
  //     walletDevice2
  //   );

  //   let timestamp = await getBlockchainTimestamp(connection);
  //   let nonce = await getLatestNonce(lazorkitProgram, smartWallet);
  //   let plainMessage = buildCallPolicyMessage(
  //     payer.publicKey,
  //     smartWallet,
  //     nonce,
  //     timestamp,
  //     addDevice2Ix
  //   );

  //   let { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
  //     await buildFakeMessagePasskey(plainMessage);

  //   let signature = privateKey1.sign(message);

  //   const addDevice2Txn = await lazorkitProgram.callPolicyTxn({
  //     payer: payer.publicKey,
  //     smartWallet,
  //     passkeySignature: {
  //       passkeyPublicKey: passkeyPubkey1,
  //       signature64: signature,
  //       clientDataJsonRaw64: clientDataJsonRaw64,
  //       authenticatorDataRaw64: authenticatorDataRaw64,
  //     },
  //     policyInstruction: addDevice2Ix,
  //     newWalletDevice: {
  //       passkeyPublicKey: passkeyPubkey2,
  //       credentialIdBase64: credentialId,
  //     },
  //     timestamp,
  //     vaultIndex: 0,
  //   });

  //   await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     addDevice2Txn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Added second device');

  //   // Execute transaction with the second device (newly added)
  //   const transferFromSmartWalletIns = anchor.web3.SystemProgram.transfer({
  //     fromPubkey: smartWallet,
  //     toPubkey: anchor.web3.Keypair.generate().publicKey,
  //     lamports: 0.001 * anchor.web3.LAMPORTS_PER_SOL,
  //   });

  //   const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
  //     smartWalletId,
  //     passkeyPubkey2,
  //     walletDevice2,
  //     smartWallet
  //   );

  //   timestamp = await getBlockchainTimestamp(connection);
  //   nonce = await getLatestNonce(lazorkitProgram, smartWallet);
  //   plainMessage = buildExecuteMessage(
  //     payer.publicKey,
  //     smartWallet,
  //     nonce,
  //     timestamp,
  //     checkPolicyIns,
  //     transferFromSmartWalletIns
  //   );

  //   ({ message, clientDataJsonRaw64, authenticatorDataRaw64 } =
  //     await buildFakeMessagePasskey(plainMessage));

  //   signature = privateKey2.sign(message);

  //   const executeDirectTransactionTxn = await lazorkitProgram.executeTxn({
  //     payer: payer.publicKey,
  //     smartWallet: smartWallet,
  //     passkeySignature: {
  //       passkeyPublicKey: passkeyPubkey2,
  //       signature64: signature,
  //       clientDataJsonRaw64: clientDataJsonRaw64,
  //       authenticatorDataRaw64: authenticatorDataRaw64,
  //     },
  //     policyInstruction: checkPolicyIns,
  //     cpiInstruction: transferFromSmartWalletIns,
  //     vaultIndex: 0,
  //     timestamp,
  //   });

  //   const executeSig = await anchor.web3.sendAndConfirmTransaction(
  //     connection,
  //     executeDirectTransactionTxn as anchor.web3.Transaction,
  //     [payer]
  //   );

  //   console.log('Execute transaction with newly added device: ', executeSig);
  // });
});
