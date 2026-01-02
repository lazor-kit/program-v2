import * as anchor from '@coral-xyz/anchor';
import { DefaultPolicyClient } from '../defaultPolicy';
import { WalletPdaFactory } from './walletPdas';
import * as types from '../../types';

type PublicKey = anchor.web3.PublicKey;
type TransactionInstruction = anchor.web3.TransactionInstruction;
type BN = anchor.BN;

interface ExecutePolicyContext {
  provided?: TransactionInstruction;
  smartWallet: PublicKey;
  authority: PublicKey;
  policyData: Buffer;
}

interface CreatePolicyContext {
  provided?: TransactionInstruction;
  smartWallet: PublicKey;
  authority: PublicKey;
}

/**
 * Resolves policy instructions by either returning a provided instruction or
 * lazily falling back to the default policy program.
 */
export class PolicyInstructionResolver {
  constructor(
    private readonly policyClient: DefaultPolicyClient,
    private readonly walletPdas: WalletPdaFactory
  ) { }

  async resolveForExecute({
    provided,
    smartWallet,
    authority,
    policyData
  }: ExecutePolicyContext): Promise<TransactionInstruction> {
    if (provided !== undefined) {
      return provided;
    }

    return this.policyClient.buildCheckPolicyIx({
      authority,
      smartWallet,
      policyData,
    });
  }

  async resolveForCreate({
    provided,
    smartWallet,
    authority,
  }: CreatePolicyContext): Promise<TransactionInstruction> {
    if (provided !== undefined) {
      return provided;
    }

    return this.policyClient.buildInitPolicyIx({
      authority,
      smartWallet,
    });
  }
}
