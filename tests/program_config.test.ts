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

  describe('Manage vault', () => {
    it('Deposit success', async () => {
      const txn = await lazorkitProgram.createManageVaultTransaction({
        payer: payer.publicKey,
        action: 'deposit',
        amount: new anchor.BN(1000000000),
        destination: payer.publicKey,
        vaultIndex: 0,
      });

      txn.sign([payer]);

      const sig = await connection.sendTransaction(txn);
      await connection.confirmTransaction(sig);

      console.log('Manage vault: ', sig);
    });

    it('Deposit failed', async () => {
      const txn = await lazorkitProgram.createManageVaultTransaction({
        payer: payer.publicKey,
        action: 'deposit',
        amount: new anchor.BN(10000),
        destination: payer.publicKey,
        vaultIndex: lazorkitProgram.generateVaultIndex(),
      });

      txn.sign([payer]);

      try {
        await connection.sendTransaction(txn);
      } catch (error) {
        expect(String(error).includes('InsufficientBalanceForFee')).to.be.true;
      }
    });

    it('Withdraw success', async () => {
      const vaultIndex = lazorkitProgram.generateVaultIndex();
      // deposit some SOL to the vault
      const depositTxn = await lazorkitProgram.createManageVaultTransaction({
        payer: payer.publicKey,
        action: 'deposit',
        amount: new anchor.BN(1000000000),
        destination: payer.publicKey,
        vaultIndex: vaultIndex,
      });

      depositTxn.sign([payer]);

      const depositSig = await connection.sendTransaction(depositTxn);
      await connection.confirmTransaction(depositSig);

      const withdrawTxn = await lazorkitProgram.createManageVaultTransaction({
        payer: payer.publicKey,
        action: 'withdraw',
        amount: new anchor.BN(10000),
        destination: payer.publicKey,
        vaultIndex: vaultIndex,
      });

      withdrawTxn.sign([payer]);

      const sig = await connection.sendTransaction(withdrawTxn);
      await connection.confirmTransaction(sig);

      console.log('Manage vault: ', sig);
    });

    it('Withdraw failed', async () => {
      const vaultIndex = lazorkitProgram.generateVaultIndex();
      const depositTxn = await lazorkitProgram.createManageVaultTransaction({
        payer: payer.publicKey,
        action: 'deposit',
        amount: new anchor.BN(1000000000),
        destination: payer.publicKey,
        vaultIndex: vaultIndex,
      });

      depositTxn.sign([payer]);

      const depositSig = await connection.sendTransaction(depositTxn);
      await connection.confirmTransaction(depositSig);

      const withdrawTxn = await lazorkitProgram.createManageVaultTransaction({
        payer: payer.publicKey,
        action: 'withdraw',
        amount: new anchor.BN(1000000000),
        destination: payer.publicKey,
        vaultIndex: vaultIndex,
      });

      withdrawTxn.sign([payer]);

      try {
        await connection.sendTransaction(withdrawTxn);
      } catch (error) {
        expect(String(error).includes('InsufficientVaultBalance')).to.be.true;
      }
    });
  });
});
