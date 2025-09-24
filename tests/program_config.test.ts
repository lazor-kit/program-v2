import * as anchor from '@coral-xyz/anchor';
import { expect } from 'chai';
import * as dotenv from 'dotenv';
import { bs58 } from '@coral-xyz/anchor/dist/cjs/utils/bytes';
import { LazorkitClient } from '../contract-integration';
import { LAMPORTS_PER_SOL } from '@solana/web3.js';
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

    const config = await connection.getAccountInfo(
      lazorkitProgram.getConfigPubkey()
    );

    if (config === null) {
      const ix = await lazorkitProgram.buildInitializeProgramIns(
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
      const txn = await lazorkitProgram.manageVaultTxn({
        payer: payer.publicKey,
        action: 'deposit',
        amount: new anchor.BN(0.001 * LAMPORTS_PER_SOL),
        destination: payer.publicKey,
        vaultIndex: 0,
      });

      const sig = await anchor.web3.sendAndConfirmTransaction(
        connection,
        txn as anchor.web3.Transaction,
        [payer]
      );

      console.log('Manage vault: ', sig);
    });

    it('Deposit failed', async () => {
      const txn = await lazorkitProgram.manageVaultTxn({
        payer: payer.publicKey,
        action: 'deposit',
        amount: new anchor.BN(1000),
        destination: payer.publicKey,
        vaultIndex: lazorkitProgram.generateVaultIndex(),
      });

      try {
        await anchor.web3.sendAndConfirmTransaction(
          connection,
          txn as anchor.web3.Transaction,
          [payer]
        );
      } catch (error) {
        expect(String(error).includes('InsufficientBalanceForFee')).to.be.true;
      }
    });

    it('Withdraw success', async () => {
      const vaultIndex = lazorkitProgram.generateVaultIndex();
      // deposit some SOL to the vault
      const depositTxn = await lazorkitProgram.manageVaultTxn({
        payer: payer.publicKey,
        action: 'deposit',
        amount: new anchor.BN(0.001 * LAMPORTS_PER_SOL),
        destination: payer.publicKey,
        vaultIndex: vaultIndex,
      });

      await anchor.web3.sendAndConfirmTransaction(
        connection,
        depositTxn as anchor.web3.Transaction,
        [payer]
      );

      const withdrawTxn = await lazorkitProgram.manageVaultTxn({
        payer: payer.publicKey,
        action: 'withdraw',
        amount: new anchor.BN(10000),
        destination: payer.publicKey,
        vaultIndex: vaultIndex,
      });

      const sig = await anchor.web3.sendAndConfirmTransaction(
        connection,
        withdrawTxn as anchor.web3.Transaction,
        [payer]
      );

      console.log('Manage vault: ', sig);
    });

    it('Withdraw failed', async () => {
      const vaultIndex = lazorkitProgram.generateVaultIndex();
      const depositTxn = await lazorkitProgram.manageVaultTxn({
        payer: payer.publicKey,
        action: 'deposit',
        amount: new anchor.BN(0.001 * LAMPORTS_PER_SOL),
        destination: payer.publicKey,
        vaultIndex: vaultIndex,
      });

      await anchor.web3.sendAndConfirmTransaction(
        connection,
        depositTxn as anchor.web3.Transaction,
        [payer]
      );

      const withdrawTxn = await lazorkitProgram.manageVaultTxn({
        payer: payer.publicKey,
        action: 'withdraw',
        amount: new anchor.BN(0.001 * LAMPORTS_PER_SOL - 1000),
        destination: payer.publicKey,
        vaultIndex: vaultIndex,
      });

      try {
        await anchor.web3.sendAndConfirmTransaction(
          connection,
          withdrawTxn as anchor.web3.Transaction,
          [payer]
        );
      } catch (error) {
        expect(String(error).includes('InsufficientVaultBalance')).to.be.true;
      }
    });
  });
});
