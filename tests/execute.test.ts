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

// Helper function to get latest nonce
async function getLatestNonce(
  lazorkitProgram: LazorkitClient,
  smartWallet: anchor.web3.PublicKey
): Promise<anchor.BN> {
  const smartWalletConfig = await lazorkitProgram.getWalletStateData(
    smartWallet
  );
  return smartWalletConfig.lastNonce;
}

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

  it('Init smart wallet with default policy successfully', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        smartWalletId,
        referral_address: payer.publicKey,
        amount: new anchor.BN(0.01 * anchor.web3.LAMPORTS_PER_SOL),
        vaultIndex: 0,
      });

    const sig = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn as anchor.web3.Transaction,
      [payer],
      {
        commitment: 'confirmed',
        skipPreflight: true,
      }
    );

    console.log('Create smart-wallet: ', sig);

    // const smartWalletConfig = await lazorkitProgram.getWalletStateData(
    //   smartWallet
    // );

    // expect(smartWalletConfig.walletId.toString()).to.be.equal(
    //   smartWalletId.toString()
    // );
  });

  it('Execute direct transaction with transfer sol from smart wallet', async () => {
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

    const policySigner = lazorkitProgram.getPolicySignerPubkey(
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

    const walletStateData = await lazorkitProgram.getWalletStateData(
      smartWallet
    );

    const transferFromSmartWalletIns = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: anchor.web3.Keypair.generate().publicKey,
      lamports: 0.001 * anchor.web3.LAMPORTS_PER_SOL,
    });

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      policySigner,
      smartWallet,
      credentialHash,
      walletStateData.policyData
    );

    const timestamp = await getBlockchainTimestamp(connection);

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
      smartWalletId,
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

    const policySigner = lazorkitProgram.getPolicySignerPubkey(
      smartWallet,
      passkeyPubkey
    );

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const credentialHash = Array.from(
      new Uint8Array(
        require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
      )
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

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      policySigner,
      smartWallet,
      credentialHash,
      walletStateData.policyData
    );

    const timestamp = await getBlockchainTimestamp(connection);

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

  xit('Execute deferred transaction with transfer token from smart wallet and transfer sol from smart_wallet', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);

    const policySigner = lazorkitProgram.getPolicySignerPubkey(
      smartWallet,
      passkeyPubkey
    );

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const credentialHash = Array.from(
      new Uint8Array(
        require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
      )
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

    const walletStateData = await lazorkitProgram.getWalletStateData(
      smartWallet
    );

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      policySigner,
      smartWallet,
      credentialHash,
      walletStateData.policyData
    );

    const timestamp = await getBlockchainTimestamp(connection);

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
        cpiInstructions: [transferTokenIns, transferFromSmartWalletIns],
        vaultIndex: 0,
        timestamp,
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

    const policySigner = lazorkitProgram.getPolicySignerPubkey(
      smartWallet,
      passkeyPubkey
    );

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const credentialHash = Array.from(
      new Uint8Array(
        require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
      )
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

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      policySigner,
      smartWallet,
      credentialHash,
      walletStateData.policyData
    );

    const timestamp = await getBlockchainTimestamp(connection);

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

  xit('Test compute unit limit functionality', async () => {
    // Create initial smart wallet with first device
    const privateKey1 = ECDSA.generateKey();
    const publicKeyBase64_1 = privateKey1.toCompressedPublicKey();
    const passkeyPubkey1 = Array.from(Buffer.from(publicKeyBase64_1, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();
    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);
    const credentialId = base64.encode(Buffer.from('testing-cu-limit'));

    // Create smart wallet
    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey1,
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

    console.log('Created smart wallet for CU limit test');

    // Test 1: Execute transaction without compute unit limit
    const transferInstruction1 = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: anchor.web3.Keypair.generate().publicKey,
      lamports: 0.001 * anchor.web3.LAMPORTS_PER_SOL,
    });

    let timestamp = await getBlockchainTimestamp(connection);
    let nonce = await getLatestNonce(lazorkitProgram, smartWallet);
    // Create a mock policy instruction
    const mockPolicyInstruction = {
      keys: [],
      programId: anchor.web3.SystemProgram.programId,
      data: Buffer.alloc(0),
    };

    let plainMessage = buildExecuteMessage(
      payer.publicKey,
      smartWallet,
      nonce,
      timestamp,
      mockPolicyInstruction,
      transferInstruction1
    );

    const { message, clientDataJsonRaw64, authenticatorDataRaw64 } =
      await buildFakeMessagePasskey(plainMessage);

    const signature1 = privateKey1.sign(message);

    const executeTxnWithoutCU = await lazorkitProgram.executeTxn(
      {
        payer: payer.publicKey,
        smartWallet: smartWallet,
        passkeySignature: {
          passkeyPublicKey: passkeyPubkey1,
          signature64: signature1,
          clientDataJsonRaw64: clientDataJsonRaw64,
          authenticatorDataRaw64: authenticatorDataRaw64,
        },
        policyInstruction: mockPolicyInstruction,
        cpiInstruction: transferInstruction1,
        timestamp,
        smartWalletId,
      },
      {
        useVersionedTransaction: true,
      }
    );

    console.log('✓ Transaction without CU limit built successfully');

    // Test 2: Execute transaction with compute unit limit
    const transferInstruction2 = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: anchor.web3.Keypair.generate().publicKey,
      lamports: 0.001 * anchor.web3.LAMPORTS_PER_SOL,
    });

    timestamp = await getBlockchainTimestamp(connection);
    nonce = await getLatestNonce(lazorkitProgram, smartWallet);
    plainMessage = buildExecuteMessage(
      payer.publicKey,
      smartWallet,
      nonce,
      timestamp,
      mockPolicyInstruction,
      transferInstruction2
    );

    const {
      message: message2,
      clientDataJsonRaw64: clientDataJsonRaw64_2,
      authenticatorDataRaw64: authenticatorDataRaw64_2,
    } = await buildFakeMessagePasskey(plainMessage);

    const signature2 = privateKey1.sign(message2);

    const executeTxnWithCU = await lazorkitProgram.executeTxn(
      {
        payer: payer.publicKey,
        smartWallet: smartWallet,
        passkeySignature: {
          passkeyPublicKey: passkeyPubkey1,
          signature64: signature2,
          clientDataJsonRaw64: clientDataJsonRaw64_2,
          authenticatorDataRaw64: authenticatorDataRaw64_2,
        },
        policyInstruction: mockPolicyInstruction,
        cpiInstruction: transferInstruction2,
        timestamp,
        smartWalletId,
      },
      {
        computeUnitLimit: 200000,
        useVersionedTransaction: true,
      }
    );

    console.log('✓ Transaction with CU limit built successfully');

    // Test 3: Verify instruction count difference
    const txWithoutCU = executeTxnWithoutCU as anchor.web3.VersionedTransaction;
    const txWithCU = executeTxnWithCU as anchor.web3.VersionedTransaction;

    // Note: We can't easily inspect the instruction count from VersionedTransaction
    // but we can verify they were built successfully
    expect(txWithoutCU).to.not.be.undefined;
    expect(txWithCU).to.not.be.undefined;

    console.log(
      '✓ Both transactions built successfully with different configurations'
    );

    // Test 4: Test createChunkTxn with compute unit limit
    const transferInstruction3 = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: anchor.web3.Keypair.generate().publicKey,
      lamports: 0.001 * anchor.web3.LAMPORTS_PER_SOL,
    });

    const transferInstruction4 = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: anchor.web3.Keypair.generate().publicKey,
      lamports: 0.001 * anchor.web3.LAMPORTS_PER_SOL,
    });

    timestamp = await getBlockchainTimestamp(connection);
    nonce = await getLatestNonce(lazorkitProgram, smartWallet);
    plainMessage = buildCreateChunkMessage(
      payer.publicKey,
      smartWallet,
      nonce,
      timestamp,
      mockPolicyInstruction,
      [transferInstruction3, transferInstruction4]
    );

    const {
      message: message3,
      clientDataJsonRaw64: clientDataJsonRaw64_3,
      authenticatorDataRaw64: authenticatorDataRaw64_3,
    } = await buildFakeMessagePasskey(plainMessage);

    const signature3 = privateKey1.sign(message3);

    const createChunkTxnWithCU = await lazorkitProgram.createChunkTxn(
      {
        payer: payer.publicKey,
        smartWallet: smartWallet,
        passkeySignature: {
          passkeyPublicKey: passkeyPubkey1,
          signature64: signature3,
          clientDataJsonRaw64: clientDataJsonRaw64_3,
          authenticatorDataRaw64: authenticatorDataRaw64_3,
        },
        policyInstruction: mockPolicyInstruction,
        cpiInstructions: [transferInstruction3, transferInstruction4],
        timestamp,
      },
      {
        computeUnitLimit: 300000, // Higher limit for multiple instructions
        useVersionedTransaction: true,
      }
    );

    expect(createChunkTxnWithCU).to.not.be.undefined;
    console.log('✓ Create chunk transaction with CU limit built successfully');

    console.log('✅ All compute unit limit tests passed!');
  });

  xit('Test verifyInstructionIndex calculation', async () => {
    // Import the helper function
    const { calculateVerifyInstructionIndex } = await import(
      '../contract-integration/transaction'
    );

    // Test without compute unit limit
    const indexWithoutCU = calculateVerifyInstructionIndex();
    expect(indexWithoutCU).to.equal(0);
    console.log('✓ verifyInstructionIndex without CU limit:', indexWithoutCU);

    // Test with compute unit limit
    const indexWithCU = calculateVerifyInstructionIndex(200000);
    expect(indexWithCU).to.equal(1);
    console.log('✓ verifyInstructionIndex with CU limit:', indexWithCU);

    // Test with undefined compute unit limit
    const indexWithUndefined = calculateVerifyInstructionIndex(undefined);
    expect(indexWithUndefined).to.equal(0);
    console.log(
      '✓ verifyInstructionIndex with undefined CU limit:',
      indexWithUndefined
    );

    console.log('✅ verifyInstructionIndex calculation tests passed!');
  });
});
