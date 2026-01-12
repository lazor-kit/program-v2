/**
 * Transaction helper utilities for integration tests
 * 
 * Uses @solana/kit only (no @solana/web3.js)
 */

import type { Address, Rpc } from '@solana/kit';
import type { Instruction } from '@solana/kit';
import type {
  GetLatestBlockhashApi,
  SendTransactionApi,
  GetSignatureStatusesApi,
  RequestAirdropApi,
  GetBalanceApi,
  GetAccountInfoApi,
  GetSlotApi,
} from '@solana/rpc-api';
import {
  createSolanaRpc,
  getAddressFromPublicKey,
  createTransactionMessage,
  signTransactionMessageWithSigners,
  setTransactionMessageFeePayer,
  appendTransactionMessageInstruction,
  setTransactionMessageLifetimeUsingBlockhash,
  compileTransaction,
  signTransaction,
  getBase64EncodedWireTransaction,
  createKeyPairSignerFromBytes,
  getBase58Encoder,
  getBase58Decoder,
} from '@solana/kit';
// @ts-ignore
import nacl from 'tweetnacl';
import { LazorkitError, LazorkitErrorCode } from '../../src/errors';

/**
 * Create a test RPC client with all needed APIs
 */
export function createTestRpc(
  rpcUrl: string = 'http://localhost:8899'
): Rpc<GetLatestBlockhashApi & SendTransactionApi & GetSignatureStatusesApi & RequestAirdropApi & GetBalanceApi & GetAccountInfoApi & GetSlotApi> {
  return createSolanaRpc(rpcUrl);
}

/**
 * Generate a test keypair using Web Crypto API
 * 
 * Note: We use Web Crypto API directly since @solana/kit's generateKeyPair
 * returns CryptoKeyPair which requires additional conversion.
 */
export async function generateTestKeypair(): Promise<{
  publicKey: Address;
  privateKey: Uint8Array;
}> {
  // Generate Ed25519 keypair using Web Crypto API
  const keyPair = await crypto.subtle.generateKey(
    {
      name: 'Ed25519',
      namedCurve: 'Ed25519',
    },
    true, // extractable
    ['sign', 'verify']
  );

  // Export private key
  const privateKeyPkcs8 = await crypto.subtle.exportKey('pkcs8', keyPair.privateKey);
  const privateKeyPkcs8Bytes = new Uint8Array(privateKeyPkcs8);

  console.log('[DEBUG] PKCS8 Length:', privateKeyPkcs8Bytes.length);
  // console.log('[DEBUG] PKCS8 Bytes:', Array.from(privateKeyPkcs8Bytes).map(b => b.toString(16).padStart(2, '0')).join(''));

  // Extract 32-byte private scalar from 48-byte PKCS#8 (remove 16-byte header)
  // Header: 302e020100300506032b657004220420
  const privateKeyBytes = privateKeyPkcs8Bytes.slice(16);

  // Export public key as raw to get bytes
  const publicKeyRaw = await crypto.subtle.exportKey('raw', keyPair.publicKey);
  const publicKeyBytes = new Uint8Array(publicKeyRaw);

  console.log('[DEBUG] Private Scalar Length:', privateKeyBytes.length);
  console.log('[DEBUG] Public Key Length:', publicKeyBytes.length);

  // Combine to create 64-byte secret key (private + public)
  const secretKey = new Uint8Array(64);
  secretKey.set(privateKeyBytes);
  secretKey.set(publicKeyBytes, 32);

  // Get address from public key using @solana/kit
  const publicKeyAddress = await getAddressFromPublicKey(keyPair.publicKey);

  return {
    publicKey: publicKeyAddress,
    privateKey: secretKey, // Return 64-byte secret key
  };
}

/**
 * Create keypair from private key bytes
 * 
 * Note: This is a simplified implementation. In production, you should
 * properly derive the public key from the private key.
 */
export async function createKeypairFromPrivateKey(privateKey: Uint8Array): Promise<{
  publicKey: Address;
  privateKey: Uint8Array;
}> {
  // Import private key using Web Crypto API
  // Convert Uint8Array to ArrayBuffer for importKey
  const privateKeyBuffer = privateKey.buffer.slice(
    privateKey.byteOffset,
    privateKey.byteOffset + privateKey.byteLength
  ) as ArrayBuffer;

  const importedPrivateKey = await crypto.subtle.importKey(
    'pkcs8',
    privateKeyBuffer,
    {
      name: 'Ed25519',
      namedCurve: 'Ed25519',
    },
    true,
    ['sign']
  );

  // For Ed25519, we need to derive the public key from the private key
  // This is complex, so for testing purposes, we'll generate a new keypair
  // In production, you should use proper Ed25519 public key derivation

  // For now, return a placeholder - this function may not be used in tests
  // If needed, implement proper Ed25519 public key derivation
  throw new Error('createKeypairFromPrivateKey not yet fully implemented. Use generateTestKeypair() instead.');
}

/**
 * Request airdrop for testing
 */
export async function requestAirdrop(
  rpc: Rpc<RequestAirdropApi & GetSignatureStatusesApi>,
  address: Address,
  amount: bigint = 2_000_000_000n // 2 SOL
): Promise<string> {
  try {
    // Convert bigint to Lamports branded type
    const signature = await rpc.requestAirdrop(address, amount as any).send();

    // Wait for confirmation
    await waitForConfirmation(rpc, signature, 'confirmed');

    return signature;
  } catch (error) {
    throw LazorkitError.fromRpcError(error);
  }
}

/**
 * Wait for transaction confirmation
 */
export async function waitForConfirmation(
  rpc: Rpc<GetSignatureStatusesApi>,
  signature: string,
  commitment: 'confirmed' | 'finalized' = 'confirmed',
  timeout: number = 30000
): Promise<void> {
  const startTime = Date.now();
  const pollInterval = 1000; // 1 second

  while (Date.now() - startTime < timeout) {
    try {
      const { value: statuses } = await rpc.getSignatureStatuses([signature as any]).send();
      const status = statuses?.[0];

      if (!status) {
        await new Promise(resolve => setTimeout(resolve, pollInterval));
        continue;
      }

      if (status.err) {
        throw new LazorkitError(
          LazorkitErrorCode.TransactionFailed,
          `Transaction failed: ${JSON.stringify(status.err)}`
        );
      }

      // Check commitment level
      if (commitment === 'confirmed' && status.confirmationStatus === 'confirmed') {
        return;
      }
      if (commitment === 'finalized' && status.confirmationStatus === 'finalized') {
        return;
      }

      await new Promise(resolve => setTimeout(resolve, pollInterval));
    } catch (error) {
      if (error instanceof LazorkitError) {
        throw error;
      }
      // Continue polling on other errors
      await new Promise(resolve => setTimeout(resolve, pollInterval));
    }
  }

  throw new LazorkitError(
    LazorkitErrorCode.RpcError,
    `Transaction confirmation timeout after ${timeout}ms`
  );
}

/**
 * Create a signer from keypair for @solana/kit transaction signing
 */
async function createSignerFromKeypair(keypair: {
  publicKey: Address;
  privateKey: Uint8Array;
}): Promise<{
  address: Address;
  signMessages: (messages: Uint8Array[]) => Promise<Uint8Array[]>;
}> {
  // Import private key as CryptoKey for signing
  const privateKeyBuffer = keypair.privateKey.buffer.slice(
    keypair.privateKey.byteOffset,
    keypair.privateKey.byteOffset + keypair.privateKey.byteLength
  ) as ArrayBuffer;

  const importedKey = await crypto.subtle.importKey(
    'pkcs8',
    privateKeyBuffer,
    {
      name: 'Ed25519',
      namedCurve: 'Ed25519',
    },
    false, // not extractable
    ['sign']
  );

  return {
    address: keypair.publicKey,
    signMessages: async (messages: Uint8Array[]): Promise<Uint8Array[]> => {
      console.log(`[DEBUG] Signing ${messages.length} messages for ${keypair.publicKey}`);
      const signatures: Uint8Array[] = [];
      for (const message of messages) {
        const signature = await crypto.subtle.sign(
          { name: 'Ed25519' },
          importedKey,
          message.buffer.slice(
            message.byteOffset,
            message.byteOffset + message.byteLength
          ) as ArrayBuffer
        );
        console.log(`[DEBUG] Generated signature length: ${signature.byteLength}`);
        signatures.push(new Uint8Array(signature));
      }
      return signatures;
    },
  };
}

/**
 * Create a legacy signer using tweetnacl to bypass @solana/kit issues
 */
async function createLegacySigner(secretKey: Uint8Array) {
  const keyPair = nacl.sign.keyPair.fromSecretKey(secretKey);
  const base58 = getBase58Decoder();
  const address = base58.decode(keyPair.publicKey) as Address;

  return {
    address: address, // string
    signMessages: async (messages: Uint8Array[]) => {
      return messages.map(msg => {
        const signature = nacl.sign.detached(msg, keyPair.secretKey);
        return signature;
      });
    },
    // Also support transaction signing if needed, but we typically use signMessages for bytes
    signTransaction: async (tx: any) => {
      throw new Error('Not implemented');
    }
  };
}

/**
 * Build and send a transaction using @solana/kit
 * 
 * @param rpc - RPC client
 * @param instructions - Array of @solana/kit instructions
 * @param payer - Fee payer keypair
 * @param additionalSigners - Additional signers
 * @returns Transaction signature
 */
export async function buildAndSendTransactionFixed(
  rpc: Rpc<GetLatestBlockhashApi & SendTransactionApi & GetSignatureStatusesApi>,
  instructions: Instruction[],
  payer: { publicKey: Address; privateKey: Uint8Array },
  additionalSigners: Array<{ publicKey: Address; privateKey: Uint8Array }> = []
): Promise<string> {
  try {
    // Get latest blockhash
    const { value: blockhash } = await rpc.getLatestBlockhash().send();

    // Create Signer using legacy method to avoid @solana/kit issues
    console.error('[DEBUG] Creating legacy signer...');
    let payerSigner;
    try {
      payerSigner = await createLegacySigner(payer.privateKey);

      // Immediate Test
      const dummy = new Uint8Array([1, 2, 3]);
      await payerSigner.signMessages([dummy]);
      console.error('[DEBUG] Legacy Signer Test: OK');
    } catch (e) {
      console.error('[DEBUG] Legacy Signer Failed:', e);
      throw e;
    }

    const additionalSignersList = await Promise.all(
      additionalSigners.map(s => createLegacySigner(s.privateKey))
    );
    const signers = [payerSigner, ...additionalSignersList];

    // Build transaction message step by step
    // Start with empty transaction message
    let transactionMessage: any = createTransactionMessage({ version: 'legacy' });

    // Set fee payer as ADDRESS string
    transactionMessage = setTransactionMessageFeePayer(payerSigner.address, transactionMessage);

    // Append all instructions before setting lifetime
    for (const instruction of instructions) {
      transactionMessage = appendTransactionMessageInstruction(instruction, transactionMessage);
    }

    // Set lifetime using blockhash (must be last)
    transactionMessage = setTransactionMessageLifetimeUsingBlockhash(blockhash, transactionMessage);

    // Manually compile and sign to debug/bypass wrapper issues
    console.error('[DEBUG] Compiling transaction...');
    const compiledTransaction = compileTransaction(transactionMessage);

    console.error('[DEBUG] Manually signing transaction...');
    const messageBytes = compiledTransaction.messageBytes;

    if (!messageBytes) {
      throw new Error('STOP: compiledTransaction.messageBytes is undefined');
    }

    // Force conversion to Uint8Array to be safe
    const messageBytesArray = new Uint8Array(messageBytes);

    // Sign with all signers
    const signatures: Record<string, Uint8Array> = {};

    for (const signer of signers) {
      // console.error(`[DEBUG] Signing with ${signer.address}`);
      try {
        const [signature] = await signer.signMessages([messageBytesArray]);
        signatures[signer.address] = signature;
      } catch (signErr) {
        console.error(`[DEBUG] Signer ${signer.address} FAILED manual sign:`, signErr);
        throw signErr;
      }
    }

    const signedTransaction = {
      ...compiledTransaction,
      signatures: {
        ...compiledTransaction.signatures,
        ...signatures,
      }
    };

    console.error('[DEBUG] Signing complete.');

    const transactionForEncoding = signedTransaction;

    // Encode transaction to wire format (base64)
    const encodedTransaction = getBase64EncodedWireTransaction(transactionForEncoding as any);

    // Send transaction
    const signature = await rpc.sendTransaction(encodedTransaction, {
      encoding: 'base64',
      skipPreflight: false,
      maxRetries: 0n,
    }).send();

    // Wait for confirmation
    await waitForConfirmation(rpc, signature, 'confirmed');

    return signature;
  } catch (error) {
    throw LazorkitError.fromRpcError(error);
  }
}

/**
 * Generate a test keypair and fund it
 */
export async function createFundedKeypair(
  rpc: Rpc<RequestAirdropApi & GetSignatureStatusesApi & GetBalanceApi>,
  amount: bigint = 2_000_000_000n // 2 SOL
): Promise<{ publicKey: Address; privateKey: Uint8Array }> {
  const keypair = await generateTestKeypair();

  // Request airdrop
  await requestAirdrop(rpc, keypair.publicKey, amount);

  return keypair;
}
