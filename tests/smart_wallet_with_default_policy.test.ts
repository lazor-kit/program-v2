import * as anchor from '@coral-xyz/anchor';
import ECDSA from 'ecdsa-secp256r1';
import { expect } from 'chai';
import * as dotenv from 'dotenv';
import { base64, bs58 } from '@coral-xyz/anchor/dist/cjs/utils/bytes';
import {
  buildCreateSessionMessage,
  DefaultPolicyClient,
  LazorkitClient,
} from '../contract-integration';
import { sha256 } from 'js-sha256';
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

    const programConfig = await connection.getAccountInfo(
      lazorkitProgram.programConfigPda()
    );

    if (programConfig === null) {
      const ix = await lazorkitProgram.buildInitializeProgramInstruction(
        payer.publicKey
      );
      const txn = new anchor.web3.Transaction().add(ix);

      const sig = await anchor.web3.sendAndConfirmTransaction(
        connection,
        txn,
        [payer],
        {
          commitment: 'confirmed',
          skipPreflight: true,
        }
      );

      console.log('Initialize txn: ', sig);
    }
  });

  xit('Init smart wallet with default policy successfully', async () => {
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
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        smartWalletId,
        amount: new anchor.BN(0.001392 * anchor.web3.LAMPORTS_PER_SOL),
      });

    const sig = await anchor.web3.sendAndConfirmTransaction(
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

    expect(smartWalletData.walletId.toString()).to.be.equal(
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

  it('Execute direct transaction', async () => {
    const privateKey = ECDSA.generateKey();

    const publicKeyBase64 = privateKey.toCompressedPublicKey();

    const passkeyPubkey = Array.from(Buffer.from(publicKeyBase64, 'base64'));

    const smartWalletId = lazorkitProgram.generateWalletId();

    const smartWallet = lazorkitProgram.smartWalletPda(smartWalletId);

    const credentialId = base64.encode(Buffer.from('testing')); // random string

    const walletDevice = lazorkitProgram.walletDevicePda(
      smartWallet,
      passkeyPubkey
    );

    const { transaction: createSmartWalletTxn } =
      await lazorkitProgram.createSmartWalletTransaction({
        payer: payer.publicKey,
        passkeyPublicKey: passkeyPubkey,
        credentialIdBase64: credentialId,
        policyInstruction: null,
        smartWalletId,
        amount: new anchor.BN(0.001392 * anchor.web3.LAMPORTS_PER_SOL).add(
          new anchor.BN(890880)
        ),
      });

    const sig = await anchor.web3.sendAndConfirmTransaction(
      connection,
      createSmartWalletTxn,
      [payer],
      {
        commitment: 'confirmed',
        skipPreflight: true,
      }
    );

    const transferFromSmartWalletIns = anchor.web3.SystemProgram.transfer({
      fromPubkey: smartWallet,
      toPubkey: anchor.web3.Keypair.generate().publicKey,
      lamports: 0 * anchor.web3.LAMPORTS_PER_SOL,
    });

    const checkPolicyIns = await defaultPolicyClient.buildCheckPolicyIx(
      smartWalletId,
      passkeyPubkey,
      walletDevice,
      smartWallet
    );

    const plainMessage = buildCreateSessionMessage(
      smartWallet,
      new anchor.BN(0),
      new anchor.BN(Math.floor(Date.now() / 1000)),
      checkPolicyIns,
      [transferFromSmartWalletIns]
    );

    const clientDataJsonRaw = Buffer.from(
      new Uint8Array(
        new TextEncoder().encode(
          JSON.stringify({
            type: 'webauthn.get',
            challenge: bytesToBase64UrlNoPad(asUint8Array(plainMessage)),
            origin: 'https://example.com',
          })
        ).buffer
      )
    );

    const authenticatorDataRaw = Buffer.from([1, 2, 3]);

    const message = Buffer.concat([
      authenticatorDataRaw,
      Buffer.from(sha256.arrayBuffer(clientDataJsonRaw)),
    ]);

    const signature = privateKey.sign(message);

    const executeDirectTransactionTxn =
      await lazorkitProgram.createExecuteDirectTransaction({
        payer: payer.publicKey,
        smartWallet: smartWallet,
        passkeySignature: {
          passkeyPublicKey: passkeyPubkey,
          signature64: signature,
          clientDataJsonRaw64: clientDataJsonRaw.toString('base64'),
          authenticatorDataRaw64: authenticatorDataRaw.toString('base64'),
        },
        policyInstruction: null,
        cpiInstruction: transferFromSmartWalletIns,
      });

    executeDirectTransactionTxn.sign([payer]);

    const sig2 = await connection.sendTransaction(executeDirectTransactionTxn, {
      skipPreflight: true,
    });

    console.log('Execute direct transaction: ', sig2);
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

    await anchor.web3.sendAndConfirmTransaction(connection, txn, [payer], {
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
          lazorkitProgram.programConfigPda(),
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

    const sig1 = await anchor.web3.sendAndConfirmTransaction(
      connection,
      txn1,
      [payer],
      {
        commitment: 'confirmed',
      }
    );

    console.log('Extend lookup table: ', sig1);
  });
});

function asUint8Array(
  input: Buffer | ArrayBuffer | ArrayBufferView | Uint8Array
): Uint8Array {
  // Node Buffer?
  // @ts-ignore
  if (
    typeof Buffer !== 'undefined' &&
    typeof (Buffer as any).isBuffer === 'function' &&
    // @ts-ignore
    (Buffer as any).isBuffer(input)
  ) {
    return new Uint8Array(input as any);
  }
  // Đã là Uint8Array
  if (input instanceof Uint8Array) return input;
  // TypedArray/DataView
  if (ArrayBuffer.isView(input)) {
    const v = input as ArrayBufferView;
    return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
  }
  // ArrayBuffer thuần
  if (input instanceof ArrayBuffer) return new Uint8Array(input);
  throw new TypeError('Unsupported byte input');
}

function bytesToBase64UrlNoPad(u8: Uint8Array): string {
  // @ts-ignore
  if (typeof Buffer !== 'undefined') {
    return Buffer.from(u8)
      .toString('base64')
      .replace(/\+/g, '-')
      .replace(/\//g, '_')
      .replace(/=+$/g, '');
  }
  // Browser fallback
  let bin = '';
  for (let i = 0; i < u8.length; i++) bin += String.fromCharCode(u8[i]);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}
