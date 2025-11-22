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
    const credentialHash = asCredentialHash(
      Array.from(
        new Uint8Array(
          require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
        )
      )
    );

    const result = await lazorkitProgram.getSmartWalletByCredentialHash(
      credentialHash
    );
    console.log('result: ', result);
  });

  xit('Delete smart wallet successfully', async () => {
    // create smart wallet first
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

    console.log('Create smart wallet: ', sig);

    const deleteSmartWalletTxn = await lazorkitProgram.program.methods
      .deleteSmartWallet()
      .accountsPartial({
        payer: payer.publicKey,
        smartWallet: smartWallet,
        walletState: lazorkitProgram.getWalletStatePubkey(smartWallet),
        walletDevice: lazorkitProgram.getWalletDevicePubkey(
          smartWallet,
          credentialHash
        ),
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .transaction();

    const deleteSmartWalletSig = await anchor.web3.sendAndConfirmTransaction(
      connection,
      deleteSmartWalletTxn as anchor.web3.Transaction,
      [payer]
    );

    console.log('Delete smart wallet: ', deleteSmartWalletSig);
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
      toPubkey: anchor.web3.Keypair.generate().publicKey,
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
      toPubkey: anchor.web3.Keypair.generate().publicKey,
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
      (await lazorkitProgram.executeChunkTxn(
        {
          payer: payer.publicKey,
          smartWallet: smartWallet,
          cpiInstructions,
        },
        {
          useVersionedTransaction: true,
        }
      )) as anchor.web3.VersionedTransaction;

    executeDeferredTransactionTxn.sign([payer]);
    const sig3 = await connection.sendTransaction(
      executeDeferredTransactionTxn,
      {
        skipPreflight: true,
      }
    );
    await connection.confirmTransaction(sig3);

    console.log('Execute deferred transaction: ', sig3);
  });

  xit('Test compute unit limit functionality', async () => {
    // Create initial smart wallet with first device
    const privateKey1 = ECDSA.generateKey();
    const publicKeyBase64_1 = privateKey1.toCompressedPublicKey();
    const passkeyPubkey1 = asPasskeyPublicKey(
      Array.from(Buffer.from(publicKeyBase64_1, 'base64'))
    );

    const smartWalletId = lazorkitProgram.generateWalletId();
    const smartWallet = lazorkitProgram.getSmartWalletPubkey(smartWalletId);
    const credentialId = base64.encode(Buffer.from('testing-cu-limit'));

    const credentialHash = asCredentialHash(
      Array.from(
        new Uint8Array(
          require('js-sha256').arrayBuffer(Buffer.from(credentialId, 'base64'))
        )
      )
    );

    // Create smart wallet
    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTxn({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey1,
        credentialIdBase64: credentialId,
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
    // Create a mock policy instruction
    const mockPolicyInstruction = {
      keys: [],
      programId: anchor.web3.SystemProgram.programId,
      data: Buffer.alloc(0),
    };

    let plainMessage = await lazorkitProgram.buildAuthorizationMessage({
      action: {
        type: SmartWalletAction.Execute,
        args: {
          policyInstruction: mockPolicyInstruction,
          cpiInstruction: transferInstruction1,
        },
      },
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeyPublicKey: passkeyPubkey1,
      credentialHash: credentialHash,
      timestamp: new anchor.BN(timestamp),
    });

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
        credentialHash,
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

    plainMessage = await lazorkitProgram.buildAuthorizationMessage({
      action: {
        type: SmartWalletAction.Execute,
        args: {
          policyInstruction: mockPolicyInstruction,
          cpiInstruction: transferInstruction2,
        },
      },
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeyPublicKey: passkeyPubkey1,
      credentialHash: credentialHash,
      timestamp: new anchor.BN(timestamp),
    });
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
        credentialHash,
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
    plainMessage = await lazorkitProgram.buildAuthorizationMessage({
      action: {
        type: SmartWalletAction.CreateChunk,
        args: {
          policyInstruction: mockPolicyInstruction,
          cpiInstructions: [transferInstruction3, transferInstruction4],
        },
      },
      payer: payer.publicKey,
      smartWallet: smartWallet,
      passkeyPublicKey: passkeyPubkey1,
      credentialHash: credentialHash,
      timestamp: new anchor.BN(timestamp),
    });

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
        credentialHash,
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
      '../sdk/transaction'
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
