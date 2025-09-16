import * as anchor from '@coral-xyz/anchor';
import { sha256 } from 'js-sha256';
import { instructionToAccountMetas } from './utils';
import { Buffer } from 'buffer';

const coder: anchor.BorshCoder = (() => {
  const idl: any = {
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
        name: 'InvokePolicyMessage',
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
        name: 'UpdatePolicyMessage',
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
        name: 'ExecueSessionMessage',
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
        name: 'AuthorizeEphemeralExecutionMessage',
        type: {
          kind: 'struct',
          fields: [
            { name: 'nonce', type: 'u64' },
            { name: 'currentTimestamp', type: 'i64' },
            { name: 'ephemeral_public_key', type: 'pubkey' },
            { name: 'expiresAt', type: 'i64' },
            { name: 'dataHash', type: { array: ['u8', 32] } },
            { name: 'accountsHash', type: { array: ['u8', 32] } },
          ],
        },
      },
    ],
  };
  return new anchor.BorshCoder(idl);
})();

function computeSingleInsAccountsHash(
  programId: anchor.web3.PublicKey,
  metas: anchor.web3.AccountMeta[],
  smartWallet: anchor.web3.PublicKey
): Uint8Array {
  const h = sha256.create();
  h.update(programId.toBytes());
  for (const m of metas) {
    h.update(m.pubkey.toBytes());
    h.update(Uint8Array.from([0])); // isSigner is always false
    h.update(
      Uint8Array.from([
        m.pubkey.toString() === smartWallet.toString() || m.isWritable ? 1 : 0,
      ])
    );
  }
  return new Uint8Array(h.arrayBuffer());
}

function computeAllInsAccountsHash(
  metas: anchor.web3.AccountMeta[],
  smartWallet: anchor.web3.PublicKey
): Uint8Array {
  // Merge duplicate accounts with the specified logic
  const accountMap = new Map<string, anchor.web3.AccountMeta>();

  for (const meta of metas) {
    const key = meta.pubkey.toString();

    if (accountMap.has(key)) {
      // Account already exists, merge properties
      const existing = accountMap.get(key)!;
      const merged: anchor.web3.AccountMeta = {
        pubkey: meta.pubkey,
        isSigner: false, // isSigner is always false
        isWritable: existing.isWritable || meta.isWritable, // true if either is true
      };
      accountMap.set(key, merged);
    } else {
      // New account, add as is
      accountMap.set(key, meta);
    }
  }

  // Convert map back to array and sort by pubkey for consistent ordering
  const mergedMetas = Array.from(accountMap.values()).sort((a, b) =>
    a.pubkey.toString().localeCompare(b.pubkey.toString())
  );

  console.log(mergedMetas);

  const h = sha256.create();
  for (const m of mergedMetas) {
    h.update(m.pubkey.toBytes());
    h.update(Uint8Array.from([0])); // isSigner is always false
    h.update(
      Uint8Array.from([
        m.pubkey.toString() === smartWallet.toString() || m.isWritable ? 1 : 0,
      ])
    );
  }
  return new Uint8Array(h.arrayBuffer());
}

export function buildExecuteMessage(
  payer: anchor.web3.PublicKey,
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  policyIns: anchor.web3.TransactionInstruction,
  cpiIns: anchor.web3.TransactionInstruction
): Buffer {
  const policyMetas = instructionToAccountMetas(policyIns);
  const policyAccountsHash = computeSingleInsAccountsHash(
    policyIns.programId,
    policyMetas,
    smartWallet
  );
  const policyDataHash = new Uint8Array(sha256.arrayBuffer(policyIns.data));

  const cpiMetas = instructionToAccountMetas(cpiIns);
  const cpiAccountsHash = computeSingleInsAccountsHash(
    cpiIns.programId,
    cpiMetas,
    smartWallet
  );
  const cpiDataHash = new Uint8Array(sha256.arrayBuffer(cpiIns.data));

  const encoded = coder.types.encode('ExecuteMessage', {
    nonce,
    currentTimestamp: now,
    policyDataHash: Array.from(policyDataHash),
    policyAccountsHash: Array.from(policyAccountsHash),
    cpiDataHash: Array.from(cpiDataHash),
    cpiAccountsHash: Array.from(cpiAccountsHash),
  });
  return Buffer.from(encoded);
}

export function buildInvokePolicyMessage(
  payer: anchor.web3.PublicKey,
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  policyIns: anchor.web3.TransactionInstruction
): Buffer {
  const policyMetas = instructionToAccountMetas(policyIns);
  const policyAccountsHash = computeSingleInsAccountsHash(
    policyIns.programId,
    policyMetas,
    smartWallet
  );
  const policyDataHash = new Uint8Array(sha256.arrayBuffer(policyIns.data));

  const encoded = coder.types.encode('InvokePolicyMessage', {
    nonce,
    currentTimestamp: now,
    policyDataHash: Array.from(policyDataHash),
    policyAccountsHash: Array.from(policyAccountsHash),
  });
  return Buffer.from(encoded);
}

export function buildUpdatePolicyMessage(
  payer: anchor.web3.PublicKey,
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  destroyPolicyIns: anchor.web3.TransactionInstruction,
  initPolicyIns: anchor.web3.TransactionInstruction
): Buffer {
  const oldMetas = instructionToAccountMetas(destroyPolicyIns);
  const oldAccountsHash = computeSingleInsAccountsHash(
    destroyPolicyIns.programId,
    oldMetas,
    smartWallet
  );
  const oldDataHash = new Uint8Array(sha256.arrayBuffer(destroyPolicyIns.data));

  const newMetas = instructionToAccountMetas(initPolicyIns);
  const newAccountsHash = computeSingleInsAccountsHash(
    initPolicyIns.programId,
    newMetas,
    smartWallet
  );
  const newDataHash = new Uint8Array(sha256.arrayBuffer(initPolicyIns.data));

  const encoded = coder.types.encode('UpdatePolicyMessage', {
    nonce,
    currentTimestamp: now,
    oldPolicyDataHash: Array.from(oldDataHash),
    oldPolicyAccountsHash: Array.from(oldAccountsHash),
    newPolicyDataHash: Array.from(newDataHash),
    newPolicyAccountsHash: Array.from(newAccountsHash),
  });
  return Buffer.from(encoded);
}

export function buildCreateSessionMessage(
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  policyIns: anchor.web3.TransactionInstruction,
  cpiInstructions: anchor.web3.TransactionInstruction[]
): Buffer {
  const policyMetas = instructionToAccountMetas(policyIns);
  const policyAccountsHash = computeSingleInsAccountsHash(
    policyIns.programId,
    policyMetas,
    smartWallet
  );
  const policyDataHash = new Uint8Array(sha256.arrayBuffer(policyIns.data));

  if (cpiInstructions.length === 1) {
    const cpiMetas = instructionToAccountMetas(cpiInstructions[0]);
    const cpiAccountsHash = computeSingleInsAccountsHash(
      cpiInstructions[0].programId,
      cpiMetas,
      smartWallet
    );
    const cpiDataHash = new Uint8Array(
      sha256.arrayBuffer(cpiInstructions[0].data)
    );
    return Buffer.from(
      coder.types.encode('ExecueSessionMessage', {
        nonce,
        currentTimestamp: now,
        policyDataHash: Array.from(policyDataHash),
        policyAccountsHash: Array.from(policyAccountsHash),
        cpiDataHash: Array.from(cpiDataHash),
        cpiAccountsHash: Array.from(cpiAccountsHash),
      })
    );
  }

  // Combine all CPI instruction data and hash it
  const allCpiData = cpiInstructions.map((ix) => Array.from(ix.data)).flat();
  const cpiDataHash = new Uint8Array(
    sha256.arrayBuffer(new Uint8Array(allCpiData))
  );

  const allMetas = cpiInstructions.flatMap((ix) =>
    instructionToAccountMetas(ix)
  );

  const cpiAccountsHash = computeAllInsAccountsHash(allMetas, smartWallet);

  const encoded = coder.types.encode('ExecueSessionMessage', {
    nonce,
    currentTimestamp: now,
    policyDataHash: Array.from(policyDataHash),
    policyAccountsHash: Array.from(policyAccountsHash),
    cpiDataHash: Array.from(cpiDataHash),
    cpiAccountsHash: Array.from(cpiAccountsHash),
  });
  return Buffer.from(encoded);
}

export function buildAuthorizeEphemeralMessage(
  payer: anchor.web3.PublicKey,
  smartWallet: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  ephemeral_public_key: anchor.web3.PublicKey,
  expiresAt: anchor.BN,
  cpiInstructions: anchor.web3.TransactionInstruction[]
): Buffer {
  // Combine all CPI instruction data and hash it
  const allCpiData = cpiInstructions.map((ix) => Array.from(ix.data)).flat();
  const dataHash = new Uint8Array(
    sha256.arrayBuffer(new Uint8Array(allCpiData))
  );

  // Combine all account metas
  const allMetas = cpiInstructions.flatMap((ix) =>
    instructionToAccountMetas(ix)
  );
  const accountsHash = computeAllInsAccountsHash(allMetas, smartWallet);

  const encoded = coder.types.encode('AuthorizeEphemeralExecutionMessage', {
    nonce,
    currentTimestamp: now,
    ephemeral_public_key,
    expiresAt,
    dataHash: Array.from(dataHash),
    accountsHash: Array.from(accountsHash),
  });
  return Buffer.from(encoded);
}
