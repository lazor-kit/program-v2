import * as anchor from '@coral-xyz/anchor';
import {
  Transaction,
  TransactionMessage,
  VersionedTransaction,
  AddressLookupTableAccount,
} from '@solana/web3.js';
import { TransactionBuilderOptions, TransactionBuilderResult } from './types';

/**
 * Builds a versioned transaction (v0) from instructions
 */
export async function buildVersionedTransaction(
  connection: anchor.web3.Connection,
  payer: anchor.web3.PublicKey,
  instructions: anchor.web3.TransactionInstruction[]
): Promise<VersionedTransaction> {
  const result = await buildTransaction(connection, payer, instructions, {
    useVersionedTransaction: true,
  });
  return result.transaction as VersionedTransaction;
}

/**
 * Builds a legacy transaction from instructions
 */
export async function buildLegacyTransaction(
  connection: anchor.web3.Connection,
  payer: anchor.web3.PublicKey,
  instructions: anchor.web3.TransactionInstruction[]
): Promise<Transaction> {
  const result = await buildTransaction(connection, payer, instructions, {
    useVersionedTransaction: false,
  });
  return result.transaction as Transaction;
}

/**
 * Combines authentication verification instruction with smart wallet instructions
 */
export function combineInstructionsWithAuth(
  authInstruction: anchor.web3.TransactionInstruction,
  smartWalletInstructions: anchor.web3.TransactionInstruction[]
): anchor.web3.TransactionInstruction[] {
  return [authInstruction, ...smartWalletInstructions];
}

/**
 * Flexible transaction builder that supports both legacy and versioned transactions
 * with optional address lookup table support
 */
export async function buildTransaction(
  connection: anchor.web3.Connection,
  payer: anchor.web3.PublicKey,
  instructions: anchor.web3.TransactionInstruction[],
  options: TransactionBuilderOptions = {}
): Promise<TransactionBuilderResult> {
  const {
    useVersionedTransaction,
    addressLookupTable,
    recentBlockhash: customBlockhash,
  } = options;

  // Auto-detect: if addressLookupTable is provided, use versioned transaction
  const shouldUseVersioned = useVersionedTransaction ?? !!addressLookupTable;

  // Get recent blockhash
  const recentBlockhash =
    customBlockhash || (await connection.getLatestBlockhash()).blockhash;

  if (shouldUseVersioned) {
    // Build versioned transaction
    const lookupTables = addressLookupTable ? [addressLookupTable] : [];

    const message = new TransactionMessage({
      payerKey: payer,
      recentBlockhash,
      instructions,
    }).compileToV0Message(lookupTables);

    const transaction = new VersionedTransaction(message);

    return {
      transaction,
      isVersioned: true,
      recentBlockhash,
    };
  } else {
    // Build legacy transaction
    const transaction = new Transaction().add(...instructions);
    transaction.feePayer = payer;
    transaction.recentBlockhash = recentBlockhash;

    return {
      transaction,
      isVersioned: false,
      recentBlockhash,
    };
  }
}
