import * as anchor from '@coral-xyz/anchor';
import { sha256 } from 'js-sha256';
import { instructionToAccountMetas } from './utils';
import { Buffer } from 'buffer';

// Type definitions for better type safety
interface MessageBase {
  nonce: anchor.BN;
  currentTimestamp: anchor.BN;
}

interface PolicyHashes {
  policyDataHash: Uint8Array;
  policyAccountsHash: Uint8Array;
}

interface CpiHashes {
  cpiDataHash: Uint8Array;
  cpiAccountsHash: Uint8Array;
}

interface ExecuteMessageData extends MessageBase, PolicyHashes, CpiHashes {}

interface CreateChunkMessageData extends MessageBase, PolicyHashes, CpiHashes {
  expiresAt: anchor.BN;
}

interface CallPolicyMessageData extends MessageBase, PolicyHashes {}

interface ChangePolicyMessageData extends MessageBase {
  oldPolicyDataHash: Uint8Array;
  oldPolicyAccountsHash: Uint8Array;
  newPolicyDataHash: Uint8Array;
  newPolicyAccountsHash: Uint8Array;
}

interface GrantPermissionMessageData extends MessageBase {
  ephemeralKey: anchor.web3.PublicKey;
  expiresAt: anchor.BN;
  dataHash: Uint8Array;
  accountsHash: Uint8Array;
}

// Optimized IDL definition with proper typing
const createMessageIdl = (): any => ({
  version: '0.1.0',
  name: 'lazorkit_msgs',
  instructions: [],
  accounts: [],
  types: [
    {
      name: 'ExecuteMessage',
      type: {
        kind: 'struct',
        fields: [
          { name: 'nonce', type: 'u64' },
          { name: 'currentTimestamp', type: 'i64' },
          { name: 'policyDataHash', type: { array: ['u8', 32] } },
          { name: 'policyAccountsHash', type: { array: ['u8', 32] } },
          { name: 'cpiDataHash', type: { array: ['u8', 32] } },
          { name: 'cpiAccountsHash', type: { array: ['u8', 32] } },
        ],
      },
    },
    {
      name: 'CreateChunkMessage',
      type: {
        kind: 'struct',
        fields: [
          { name: 'nonce', type: 'u64' },
          { name: 'currentTimestamp', type: 'i64' },
          { name: 'policyDataHash', type: { array: ['u8', 32] } },
          { name: 'policyAccountsHash', type: { array: ['u8', 32] } },
          { name: 'cpiDataHash', type: { array: ['u8', 32] } },
          { name: 'cpiAccountsHash', type: { array: ['u8', 32] } },
          { name: 'expiresAt', type: 'i64' },
        ],
      },
    },
    {
      name: 'CallPolicyMessage',
      type: {
        kind: 'struct',
        fields: [
          { name: 'nonce', type: 'u64' },
          { name: 'currentTimestamp', type: 'i64' },
          { name: 'policyDataHash', type: { array: ['u8', 32] } },
          { name: 'policyAccountsHash', type: { array: ['u8', 32] } },
        ],
      },
    },
    {
      name: 'ChangePolicyMessage',
      type: {
        kind: 'struct',
        fields: [
          { name: 'nonce', type: 'u64' },
          { name: 'currentTimestamp', type: 'i64' },
          { name: 'oldPolicyDataHash', type: { array: ['u8', 32] } },
          { name: 'oldPolicyAccountsHash', type: { array: ['u8', 32] } },
          { name: 'newPolicyDataHash', type: { array: ['u8', 32] } },
          { name: 'newPolicyAccountsHash', type: { array: ['u8', 32] } },
        ],
      },
    },
    {
      name: 'GrantPermissionMessage',
      type: {
        kind: 'struct',
        fields: [
          { name: 'nonce', type: 'u64' },
          { name: 'currentTimestamp', type: 'i64' },
          { name: 'ephemeralKey', type: 'pubkey' },
          { name: 'expiresAt', type: 'i64' },
          { name: 'dataHash', type: { array: ['u8', 32] } },
          { name: 'accountsHash', type: { array: ['u8', 32] } },
        ],
      },
    },
  ],
});

// Lazy-loaded coder for better performance
let coder: anchor.BorshCoder | null = null;
const getCoder = (): anchor.BorshCoder => {
  if (!coder) {
    coder = new anchor.BorshCoder(createMessageIdl());
  }
  return coder;
};

// Optimized hash computation with better performance
const computeHash = (data: Uint8Array): Uint8Array => {
  return new Uint8Array(sha256.arrayBuffer(data));
};

// Optimized single instruction accounts hash computation
const computeSingleInsAccountsHash = (
  programId: anchor.web3.PublicKey,
  metas: anchor.web3.AccountMeta[],
  smartWallet: anchor.web3.PublicKey
): Uint8Array => {
  const h = sha256.create();
  h.update(programId.toBytes());

  for (const meta of metas) {
    h.update(meta.pubkey.toBytes());
    h.update(Uint8Array.from([meta.isSigner ? 1 : 0]));
    h.update(
      Uint8Array.from([
        meta.pubkey.toString() === smartWallet.toString() || meta.isWritable
          ? 1
          : 0,
      ])
    );
  }

  return new Uint8Array(h.arrayBuffer());
};

// Optimized multiple instructions accounts hash computation
const computeAllInsAccountsHash = (
  metas: anchor.web3.AccountMeta[],
  smartWallet: anchor.web3.PublicKey
): Uint8Array => {
  // Use Map for O(1) lookups instead of repeated array operations
  const pubkeyProperties = new Map<
    string,
    { isSigner: boolean; isWritable: boolean }
  >();

  // Single pass to collect properties
  for (const meta of metas) {
    const key = meta.pubkey.toString();
    const existing = pubkeyProperties.get(key);

    if (existing) {
      existing.isSigner = existing.isSigner || meta.isSigner;
      existing.isWritable = existing.isWritable || meta.isWritable;
    } else {
      pubkeyProperties.set(key, {
        isSigner: meta.isSigner,
        isWritable: meta.isWritable,
      });
    }
  }

  // Create processed metas with optimized properties
  const processedMetas = metas.map((meta) => {
    const key = meta.pubkey.toString();
    const properties = pubkeyProperties.get(key)!;

    return {
      pubkey: meta.pubkey,
      isSigner: properties.isSigner,
      isWritable: properties.isWritable,
    };
  });

  const h = sha256.create();
  for (const meta of processedMetas) {
    h.update(meta.pubkey.toBytes());
    h.update(Uint8Array.from([meta.isSigner ? 1 : 0]));
    h.update(
      Uint8Array.from([
        meta.pubkey.toString() === smartWallet.toString() || meta.isWritable
          ? 1
          : 0,
      ])
    );
  }

  return new Uint8Array(h.arrayBuffer());
};

// Helper function to compute policy hashes
const computePolicyHashes = (
  policyIns: anchor.web3.TransactionInstruction,
  smartWallet: anchor.web3.PublicKey,
  allowSigner?: anchor.web3.PublicKey[]
): PolicyHashes => {
  const policyMetas = instructionToAccountMetas(policyIns, allowSigner);
  const policyAccountsHash = computeSingleInsAccountsHash(
    policyIns.programId,
    policyMetas,
    smartWallet
  );
  const policyDataHash = computeHash(policyIns.data);

  return { policyDataHash, policyAccountsHash };
};

// Helper function to compute CPI hashes for single instruction
const computeCpiHashes = (
  cpiIns: anchor.web3.TransactionInstruction,
  smartWallet: anchor.web3.PublicKey,
  allowSigner?: anchor.web3.PublicKey[]
): CpiHashes => {
  const cpiMetas = instructionToAccountMetas(cpiIns, allowSigner);
  const cpiAccountsHash = computeSingleInsAccountsHash(
    cpiIns.programId,
    cpiMetas,
    smartWallet
  );
  const cpiDataHash = computeHash(cpiIns.data);

  return { cpiDataHash, cpiAccountsHash };
};

// Helper function to compute CPI hashes for multiple instructions
const computeMultipleCpiHashes = (
  cpiInstructions: anchor.web3.TransactionInstruction[],
  smartWallet: anchor.web3.PublicKey,
  allowSigner?: anchor.web3.PublicKey[]
): CpiHashes => {
  // Optimized serialization without unnecessary Buffer allocations
  const lengthBuffer = Buffer.alloc(4);
  lengthBuffer.writeUInt32LE(cpiInstructions.length, 0);

  const serializedData = Buffer.concat([
    lengthBuffer,
    ...cpiInstructions.map((ix) => {
      const data = Buffer.from(ix.data);
      const dataLengthBuffer = Buffer.alloc(4);
      dataLengthBuffer.writeUInt32LE(data.length, 0);
      return Buffer.concat([dataLengthBuffer, data]);
    }),
  ]);

  const cpiDataHash = computeHash(serializedData);

  const allMetas = cpiInstructions.flatMap((ix) => [
    { pubkey: ix.programId, isSigner: false, isWritable: false },
    ...instructionToAccountMetas(ix, allowSigner),
  ]);

  const cpiAccountsHash = computeAllInsAccountsHash(allMetas, smartWallet);

  return { cpiDataHash, cpiAccountsHash };
};

// Helper function to encode message with proper error handling
const encodeMessage = <T>(messageType: string, data: T): Buffer => {
  try {
    const encoded = getCoder().types.encode(messageType, data);
    return Buffer.from(encoded);
  } catch (error) {
    throw new Error(
      `Failed to encode ${messageType}: ${
        error instanceof Error ? error.message : 'Unknown error'
      }`
    );
  }
};

// Main message building functions with optimized implementations

export function buildExecuteMessage(
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  policyIns: anchor.web3.TransactionInstruction,
  cpiIns: anchor.web3.TransactionInstruction,
  allowSigner?: anchor.web3.PublicKey[]
): Buffer {
  const policyHashes = computePolicyHashes(policyIns, smartWallet, allowSigner);
  const cpiHashes = computeCpiHashes(cpiIns, smartWallet, allowSigner);

  const messageData: ExecuteMessageData = {
    nonce,
    currentTimestamp: now,
    ...policyHashes,
    ...cpiHashes,
  };

  return encodeMessage('ExecuteMessage', {
    ...messageData,
    policyDataHash: Array.from(messageData.policyDataHash),
    policyAccountsHash: Array.from(messageData.policyAccountsHash),
    cpiDataHash: Array.from(messageData.cpiDataHash),
    cpiAccountsHash: Array.from(messageData.cpiAccountsHash),
  });
}

export function buildCallPolicyMessage(
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  policyIns: anchor.web3.TransactionInstruction,
  allowSigner?: anchor.web3.PublicKey[]
): Buffer {
  const policyHashes = computePolicyHashes(policyIns, smartWallet, allowSigner);

  const messageData: CallPolicyMessageData = {
    nonce,
    currentTimestamp: now,
    ...policyHashes,
  };

  return encodeMessage('CallPolicyMessage', {
    ...messageData,
    policyDataHash: Array.from(messageData.policyDataHash),
    policyAccountsHash: Array.from(messageData.policyAccountsHash),
  });
}

export function buildChangePolicyMessage(
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  destroyPolicyIns: anchor.web3.TransactionInstruction,
  initPolicyIns: anchor.web3.TransactionInstruction,
  allowSigner?: anchor.web3.PublicKey[]
): Buffer {
  const oldHashes = computePolicyHashes(
    destroyPolicyIns,
    smartWallet,
    allowSigner
  );
  const newHashes = computePolicyHashes(
    initPolicyIns,
    smartWallet,
    allowSigner
  );

  const messageData: ChangePolicyMessageData = {
    nonce,
    currentTimestamp: now,
    oldPolicyDataHash: oldHashes.policyDataHash,
    oldPolicyAccountsHash: oldHashes.policyAccountsHash,
    newPolicyDataHash: newHashes.policyDataHash,
    newPolicyAccountsHash: newHashes.policyAccountsHash,
  };

  return encodeMessage('ChangePolicyMessage', {
    ...messageData,
    oldPolicyDataHash: Array.from(messageData.oldPolicyDataHash),
    oldPolicyAccountsHash: Array.from(messageData.oldPolicyAccountsHash),
    newPolicyDataHash: Array.from(messageData.newPolicyDataHash),
    newPolicyAccountsHash: Array.from(messageData.newPolicyAccountsHash),
  });
}

export function buildCreateChunkMessage(
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  policyIns: anchor.web3.TransactionInstruction,
  cpiInstructions: anchor.web3.TransactionInstruction[],
  expiresAt: anchor.BN,
  allowSigner?: anchor.web3.PublicKey[]
): Buffer {
  const policyHashes = computePolicyHashes(policyIns, smartWallet, allowSigner);
  const cpiHashes = computeMultipleCpiHashes(
    cpiInstructions,
    smartWallet,
    allowSigner
  );

  const messageData: CreateChunkMessageData = {
    nonce,
    currentTimestamp: now,
    expiresAt,
    ...policyHashes,
    ...cpiHashes,
  };

  return encodeMessage('CreateChunkMessage', {
    ...messageData,
    policyDataHash: Array.from(messageData.policyDataHash),
    policyAccountsHash: Array.from(messageData.policyAccountsHash),
    cpiDataHash: Array.from(messageData.cpiDataHash),
    cpiAccountsHash: Array.from(messageData.cpiAccountsHash),
  });
}

export function buildGrantPermissionMessage(
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  ephemeralKey: anchor.web3.PublicKey,
  expiresAt: anchor.BN,
  cpiInstructions: anchor.web3.TransactionInstruction[],
  allowSigner?: anchor.web3.PublicKey[]
): Buffer {
  // Optimized data hashing
  const allCpiData = new Uint8Array(
    cpiInstructions.reduce((acc, ix) => acc + ix.data.length, 0)
  );

  let offset = 0;
  for (const ix of cpiInstructions) {
    allCpiData.set(ix.data, offset);
    offset += ix.data.length;
  }

  const dataHash = computeHash(allCpiData);

  // Optimized account metas processing
  const allMetas = cpiInstructions.flatMap((ix) =>
    instructionToAccountMetas(ix, allowSigner)
  );
  const accountsHash = computeAllInsAccountsHash(allMetas, smartWallet);

  const messageData: GrantPermissionMessageData = {
    nonce,
    currentTimestamp: now,
    ephemeralKey,
    expiresAt,
    dataHash,
    accountsHash,
  };

  return encodeMessage('GrantPermissionMessage', {
    ...messageData,
    dataHash: Array.from(messageData.dataHash),
    accountsHash: Array.from(messageData.accountsHash),
  });
}
