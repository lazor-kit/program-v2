import * as anchor from '@coral-xyz/anchor';
import DefaultPolicyIdl from '../anchor/idl/default_policy.json';
import { DefaultPolicy } from '../anchor/types/default_policy';
import { derivePolicyPda } from '../pda/defaultPolicy';
import * as types from '../types';
import {
  assertValidPublicKey,
  assertValidPasskeyPublicKey,
  assertValidCredentialHash,
  assertPositiveBN,
  assertDefined,
  ValidationError,
  toNumberArraySafe,
} from '../validation';

/**
 * Parameters for building initialize policy instruction
 */
export interface BuildInitPolicyIxParams {
  /** Policy signer PDA address (required) */
  readonly authority: anchor.web3.PublicKey;
  /** Smart wallet PDA address (required) */
  readonly smartWallet: anchor.web3.PublicKey;
}

/**
 * Parameters for building check policy instruction
 */
export interface BuildCheckPolicyIxParams {
  /** Policy signer PDA address (required) */
  readonly authority: anchor.web3.PublicKey;
  /** Smart wallet PDA address (required) */
  readonly smartWallet: anchor.web3.PublicKey;
  /** Policy data buffer (required, must be a Buffer instance) */
  readonly policyData: Buffer;
}

export class DefaultPolicyClient {
  readonly connection: anchor.web3.Connection;
  readonly program: anchor.Program<DefaultPolicy>;
  readonly programId: anchor.web3.PublicKey;

  constructor(connection: anchor.web3.Connection) {
    assertDefined(connection, 'connection');
    this.connection = connection;

    this.program = new anchor.Program<DefaultPolicy>(
      DefaultPolicyIdl as DefaultPolicy,
      {
        connection: connection,
      }
    );
    this.programId = this.program.programId;
  }

  /**
   * Gets the policy PDA for a given smart wallet
   *
   * @param smartWallet - Smart wallet PDA address
   * @returns Policy PDA address
   * @throws {ValidationError} if smartWallet is invalid
   */
  policyPda(smartWallet: anchor.web3.PublicKey): anchor.web3.PublicKey {
    assertValidPublicKey(smartWallet, 'smartWallet');
    return derivePolicyPda(this.programId, smartWallet);
  }

  /**
   * Gets the default policy data size in bytes
   *
   * @returns Policy data size in bytes
   */
  getPolicyDataSize(): number {
    return 32 + 4 + 32;
  }

  /**
   * Validates BuildInitPolicyIxParams
   */
  private validateInitPolicyParams(params: BuildInitPolicyIxParams): void {
    assertDefined(params, 'params');
    assertValidPublicKey(params.authority, 'params.authority');
    assertValidPublicKey(params.smartWallet, 'params.smartWallet');
  }

  /**
   * Builds the initialize policy instruction
   *
   * @param params - Initialize policy parameters
   * @returns Transaction instruction
   * @throws {ValidationError} if parameters are invalid
   */
  async buildInitPolicyIx(
    params: BuildInitPolicyIxParams
  ): Promise<anchor.web3.TransactionInstruction> {
    this.validateInitPolicyParams(params);

    return await this.program.methods
      .initPolicy()
      .accountsPartial({
        smartWallet: params.smartWallet,
        authority: params.authority,
      })
      .instruction();
  }

  /**
   * Validates BuildCheckPolicyIxParams
   */
  private validateCheckPolicyParams(params: BuildCheckPolicyIxParams): void {
    assertDefined(params, 'params');
    assertValidPublicKey(params.authority, 'params.authority');
    assertValidPublicKey(params.smartWallet, 'params.smartWallet');
    assertDefined(params.policyData, 'params.policyData');
    if (!Buffer.isBuffer(params.policyData)) {
      throw new ValidationError(
        'params.policyData must be a Buffer instance',
        'params.policyData'
      );
    }
  }

  /**
   * Builds the check policy instruction
   *
   * @param params - Check policy parameters
   * @returns Transaction instruction
   * @throws {ValidationError} if parameters are invalid
   */
  async buildCheckPolicyIx(
    params: BuildCheckPolicyIxParams
  ): Promise<anchor.web3.TransactionInstruction> {
    this.validateCheckPolicyParams(params);

    return await this.program.methods
      .checkPolicy(

        params.policyData
      )
      .accountsPartial({
        smartWallet: params.smartWallet,
        authority: params.authority,
      })
      .instruction();
  }
}
