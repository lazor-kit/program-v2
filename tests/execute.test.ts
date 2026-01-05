import * as anchor from '@coral-xyz/anchor';
import ECDSA from 'ecdsa-secp256r1';
import { expect } from 'chai';
import * as dotenv from 'dotenv';
import { bs58 } from '@coral-xyz/anchor/dist/cjs/utils/bytes';
import {
  DefaultPolicyClient,
  LazorkitClient,
  SmartWalletAction,
  asPasskeyPublicKey,
  asCredentialHash,
  getBlockchainTimestamp,
  getRandomBytes,
} from '../sdk';
import { createTransferInstruction } from '@solana/spl-token';
import { buildFakeMessagePasskey, createNewMint, mintTokenTo } from './utils';
dotenv.config();

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

    const credentialId = Buffer.from(getRandomBytes(32)).toString('base64');

    const credentialHash = asCredentialHash(
      Array.from(
        new Uint8Array(
          require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
        )
      )
    );

    const { transaction: createSmartWalletTxn, smartWallet } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        baseSeed: credentialHash,
        salt: new anchor.BN(0),
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

    expect(smartWalletConfig.baseSeed.toString()).to.be.equal(
      credentialHash.toString()
    );
  });

  it('Execute chunk transaction with transfer token from smart wallet', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = asPasskeyPublicKey(
      Array.from(Buffer.from(publicKeyBase64, 'base64'))
    );

    const credentialId = Buffer.from(getRandomBytes(32)).toString('base64');

    const credentialHash = asCredentialHash(
      Array.from(
        new Uint8Array(
          require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
        )
      )
    );

    const { transaction: createSmartWalletTxn, smartWallet } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        baseSeed: credentialHash,
        salt: new anchor.BN(0),
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

    const timestamp = await getBlockchainTimestamp(connection);

    const plainMessage = await lazorkitProgram.buildAuthorizationMessage({
      action: {
        type: SmartWalletAction.CreateChunk,
        args: {
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
          walletAuthority: lazorkitProgram.getWalletAuthorityPubkey(smartWallet, credentialHash),
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

    const credentialId = Buffer.from(getRandomBytes(32)).toString('base64');

    const credentialHash = asCredentialHash(
      Array.from(
        new Uint8Array(
          require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
        )
      )
    );

    const { transaction: createSmartWalletTxn, smartWallet } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        baseSeed: credentialHash,
        salt: new anchor.BN(0),
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
        walletAuthority: lazorkitProgram.getWalletAuthorityPubkey(smartWallet, credentialHash),
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
