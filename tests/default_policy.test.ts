import * as anchor from '@coral-xyz/anchor';
import ECDSA from 'ecdsa-secp256r1';
import { expect } from 'chai';
import * as dotenv from 'dotenv';
import { base64, bs58 } from '@coral-xyz/anchor/dist/cjs/utils/bytes';
import {
  buildCallPolicyMessage,
  buildCreateChunkMessage,
  buildExecuteMessage,
  DefaultPolicyClient,
  LazorkitClient,
} from '../contract-integration';
import { createTransferInstruction } from '@solana/spl-token';
import { buildFakeMessagePasskey } from './utils';
dotenv.config();

describe.skip('Test smart wallet with default policy', () => {
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

  it('Add another device to smart wallet', async () => {
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
        amount: new anchor.BN(0.001392 * anchor.web3.LAMPORTS_PER_SOL).add(
          new anchor.BN(890880).add(new anchor.BN(anchor.web3.LAMPORTS_PER_SOL))
        ),
      });

    await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    const privateKey2 = ECDSA.generateKey();

    const publicKeyBase642 = privateKey2.toCompressedPublicKey();

    const passkeyPubkey2 = Array.from(Buffer.from(publicKeyBase642, 'base64'));

    const walletDevice2 = lazorkitProgram.getWalletDevicePubkey(
      smartWallet,
      passkeyPubkey2
    );

    const addDeviceIx = await defaultPolicyClient.buildAddDeviceIx(
      smartWallet,
      walletDevice,
      walletDevice2
    );

    const timestamp = new anchor.BN(Math.floor(Date.now() / 1000));

    const plainMessage = buildCallPolicyMessage(
      payer.publicKey,
      smartWallet,
      new anchor.BN(0),
      timestamp,
      addDeviceIx
    );

    const { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
      await buildFakeMessagePasskey(plainMessage);

    const signature = privateKey.sign(message);

    const callPolicyTxn = await lazorkitProgram.callPolicyTxn({
      payer: payer.publicKey,
      smartWallet,
      passkeySignature: {
        passkeyPublicKey: passkeyPubkey,
        signature64: signature,
        clientDataJsonRaw64: clientDataJsonRaw64,
        authenticatorDataRaw64: authenticatorDataRaw64,
      },
      policyInstruction: addDeviceIx,
      newWalletDevice: {
        passkeyPublicKey: passkeyPubkey2,
        credentialIdBase64: credentialId,
      },
      timestamp,
    });

    const sig = await anchor.web3.sendAndConfirmTransaction(
      connection,
      callPolicyTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Add device txn: ', sig);
  });
});
