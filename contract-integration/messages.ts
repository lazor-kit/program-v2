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
            { name: 'ruleDataHash', type: { array: ['u8', 32] } },
            { name: 'ruleAccountsHash', type: { array: ['u8', 32] } },
            { name: 'cpiDataHash', type: { array: ['u8', 32] } },
            { name: 'cpiAccountsHash', type: { array: ['u8', 32] } },
          ],
        },
      },
      {
        name: 'CallRuleMessage',
        type: {
          kind: 'struct',
          fields: [
            { name: 'nonce', type: 'u64' },
            { name: 'currentTimestamp', type: 'i64' },
            { name: 'ruleDataHash', type: { array: ['u8', 32] } },
            { name: 'ruleAccountsHash', type: { array: ['u8', 32] } },
          ],
        },
      },
      {
        name: 'ChangeRuleMessage',
        type: {
          kind: 'struct',
          fields: [
            { name: 'nonce', type: 'u64' },
            { name: 'currentTimestamp', type: 'i64' },
            { name: 'oldRuleDataHash', type: { array: ['u8', 32] } },
            { name: 'oldRuleAccountsHash', type: { array: ['u8', 32] } },
            { name: 'newRuleDataHash', type: { array: ['u8', 32] } },
            { name: 'newRuleAccountsHash', type: { array: ['u8', 32] } },
          ],
        },
      },
    ],
  };
  return new anchor.BorshCoder(idl);
})();

function computeAccountsHash(
  programId: anchor.web3.PublicKey,
  metas: anchor.web3.AccountMeta[]
): Uint8Array {
  const h = sha256.create();
  h.update(programId.toBytes());
  for (const m of metas) {
    h.update(m.pubkey.toBytes());
    h.update(Uint8Array.from([m.isSigner ? 1 : 0]));
  }
  return new Uint8Array(h.arrayBuffer());
}

export function buildExecuteMessage(
  payer: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  ruleIns: anchor.web3.TransactionInstruction,
  cpiIns: anchor.web3.TransactionInstruction
): Buffer {
  const ruleMetas = instructionToAccountMetas(ruleIns, payer);
  const ruleAccountsHash = computeAccountsHash(ruleIns.programId, ruleMetas);
  const ruleDataHash = new Uint8Array(sha256.arrayBuffer(ruleIns.data));

  const cpiMetas = instructionToAccountMetas(cpiIns, payer);
  const cpiAccountsHash = computeAccountsHash(cpiIns.programId, cpiMetas);
  const cpiDataHash = new Uint8Array(sha256.arrayBuffer(cpiIns.data));

  const encoded = coder.types.encode('ExecuteMessage', {
    nonce,
    currentTimestamp: now,
    ruleDataHash: Array.from(ruleDataHash),
    ruleAccountsHash: Array.from(ruleAccountsHash),
    cpiDataHash: Array.from(cpiDataHash),
    cpiAccountsHash: Array.from(cpiAccountsHash),
  });
  return Buffer.from(encoded);
}

export function buildCallRuleMessage(
  payer: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  ruleIns: anchor.web3.TransactionInstruction
): Buffer {
  const ruleMetas = instructionToAccountMetas(ruleIns, payer);
  const ruleAccountsHash = computeAccountsHash(ruleIns.programId, ruleMetas);
  const ruleDataHash = new Uint8Array(sha256.arrayBuffer(ruleIns.data));

  const encoded = coder.types.encode('CallRuleMessage', {
    nonce,
    currentTimestamp: now,
    ruleDataHash: Array.from(ruleDataHash),
    ruleAccountsHash: Array.from(ruleAccountsHash),
  });
  return Buffer.from(encoded);
}

export function buildChangeRuleMessage(
  payer: anchor.web3.PublicKey,
  nonce: anchor.BN,
  now: anchor.BN,
  destroyRuleIns: anchor.web3.TransactionInstruction,
  initRuleIns: anchor.web3.TransactionInstruction
): Buffer {
  const oldMetas = instructionToAccountMetas(destroyRuleIns, payer);
  const oldAccountsHash = computeAccountsHash(destroyRuleIns.programId, oldMetas);
  const oldDataHash = new Uint8Array(sha256.arrayBuffer(destroyRuleIns.data));

  const newMetas = instructionToAccountMetas(initRuleIns, payer);
  const newAccountsHash = computeAccountsHash(initRuleIns.programId, newMetas);
  const newDataHash = new Uint8Array(sha256.arrayBuffer(initRuleIns.data));

  const encoded = coder.types.encode('ChangeRuleMessage', {
    nonce,
    currentTimestamp: now,
    oldRuleDataHash: Array.from(oldDataHash),
    oldRuleAccountsHash: Array.from(oldAccountsHash),
    newRuleDataHash: Array.from(newDataHash),
    newRuleAccountsHash: Array.from(newAccountsHash),
  });
  return Buffer.from(encoded);
}
