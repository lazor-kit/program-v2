import * as anchor from '@coral-xyz/anchor';
import { Lazorkit } from './anchor/types/lazorkit';

// ============================================================================
// Account Types (from on-chain state)
// ============================================================================
export type SmartWalletConfig = anchor.IdlTypes<Lazorkit>['smartWalletConfig'];
export type WalletDevice = anchor.IdlTypes<Lazorkit>['walletDevice'];
export type ProgramConfig = anchor.IdlTypes<Lazorkit>['config'];
export type PolicyProgramRegistry =
  anchor.IdlTypes<Lazorkit>['policyProgramRegistry'];
export type Chunk = anchor.IdlTypes<Lazorkit>['chunk'];
export type Permission = anchor.IdlTypes<Lazorkit>['permission'];

// ============================================================================
// Instruction Argument Types (from on-chain instructions)
// ============================================================================
export type CreateSmartWalletArgs =
  anchor.IdlTypes<Lazorkit>['createSmartWalletArgs'];
export type ExecuteArgs = anchor.IdlTypes<Lazorkit>['executeArgs'];
export type ChangePolicyArgs = anchor.IdlTypes<Lazorkit>['changePolicyArgs'];
export type CallPolicyArgs = anchor.IdlTypes<Lazorkit>['callPolicyArgs'];
export type CreateChunkArgs = anchor.IdlTypes<Lazorkit>['createChunkArgs'];
export type GrantPermissionArgs =
  anchor.IdlTypes<Lazorkit>['grantPermissionArgs'];
export type NewWalletDeviceArgs =
  anchor.IdlTypes<Lazorkit>['newWalletDeviceArgs'];

// ============================================================================
// Configuration Types
// ============================================================================
export type UpdateType = anchor.IdlTypes<Lazorkit>['updateType'];

// ============================================================================
// Smart Wallet Action Types
// ============================================================================
export enum SmartWalletAction {
  ChangePolicy = 'change_policy',
  CallPolicy = 'call_policy',
  Execute = 'execute',
  CreateChunk = 'create_chunk',
  ExecuteChunk = 'execute_chunk',
  GrantPermission = 'grant_permission',
  ExecuteWithPermission = 'execute_with_permission',
}

export type ArgsByAction = {
  [SmartWalletAction.Execute]: {
    policyInstruction: anchor.web3.TransactionInstruction | null;
    cpiInstruction: anchor.web3.TransactionInstruction;
  };
  [SmartWalletAction.CallPolicy]: {
    policyInstruction: anchor.web3.TransactionInstruction;
    newWalletDevice: {
      passkeyPublicKey: number[];
      credentialIdBase64: string;
    } | null;
  };
  [SmartWalletAction.ChangePolicy]: {
    destroyPolicyIns: anchor.web3.TransactionInstruction;
    initPolicyIns: anchor.web3.TransactionInstruction;
    newWalletDevice: {
      passkeyPublicKey: number[];
      credentialIdBase64: string;
    } | null;
  };
  [SmartWalletAction.CreateChunk]: {
    policyInstruction: anchor.web3.TransactionInstruction | null;
    cpiInstructions: anchor.web3.TransactionInstruction[];
    expiresAt: number;
  };
  [SmartWalletAction.ExecuteChunk]: {
    cpiInstructions: anchor.web3.TransactionInstruction[];
  };
  [SmartWalletAction.GrantPermission]: {
    ephemeral_public_key: anchor.web3.PublicKey;
    expiresAt: number;
    cpiInstructions: anchor.web3.TransactionInstruction[];
  };
  [SmartWalletAction.ExecuteWithPermission]: {
    cpiInstructions: anchor.web3.TransactionInstruction[];
  };
};

/**
 * Generic type for smart wallet action arguments.
 * Can be used for message building, SDK operations, or any other context
 * where you need to specify a smart wallet action with its arguments.
 */
export type SmartWalletActionArgs<
  K extends SmartWalletAction = SmartWalletAction
> = {
  type: K;
  args: ArgsByAction[K];
};

// ============================================================================
// Authentication Types
// ============================================================================
export interface PasskeySignature {
  passkeyPublicKey: number[];
  signature64: string;
  clientDataJsonRaw64: string;
  authenticatorDataRaw64: string;
}

export interface NewPasskeyDevice {
  passkeyPublicKey: number[];
  credentialIdBase64: string;
}

// ============================================================================
// Transaction Builder Types
// ============================================================================
export interface ManageVaultParams {
  payer: anchor.web3.PublicKey;
  amount: anchor.BN;
  action: 'deposit' | 'withdraw';
  vaultIndex: number;
  destination: anchor.web3.PublicKey;
}
export interface CreateSmartWalletParams {
  payer: anchor.web3.PublicKey;
  passkeyPublicKey: number[];
  credentialIdBase64: string;
  policyInstruction?: anchor.web3.TransactionInstruction | null;
  smartWalletId?: anchor.BN;
  referral_address?: anchor.web3.PublicKey | null;
  vaultIndex?: number;
  amount: anchor.BN;
}

export interface ExecuteParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  policyInstruction: anchor.web3.TransactionInstruction | null;
  cpiInstruction: anchor.web3.TransactionInstruction;
  vaultIndex?: number;
}

export interface CallPolicyParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  policyInstruction: anchor.web3.TransactionInstruction;
  newWalletDevice?: NewPasskeyDevice | null;
  vaultIndex?: number;
}

export interface ChangePolicyParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  destroyPolicyInstruction: anchor.web3.TransactionInstruction;
  initPolicyInstruction: anchor.web3.TransactionInstruction;
  newWalletDevice?: NewPasskeyDevice | null;
  vaultIndex?: number;
}

export interface CreateChunkParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  policyInstruction: anchor.web3.TransactionInstruction | null;
  expiresAt: number;
  vaultIndex?: number;
}

export interface ExecuteChunkParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  cpiInstructions: anchor.web3.TransactionInstruction[];
}

export interface GrantPermissionParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  ephemeral_public_key: anchor.web3.PublicKey;
  expiresAt: number;
  cpiInstructions: anchor.web3.TransactionInstruction[];
  vaultIndex?: number;
}

export interface ExecuteWithPermissionParams {
  feePayer: anchor.web3.PublicKey;
  ephemeralSigner: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  permission: anchor.web3.PublicKey;
  cpiInstructions: anchor.web3.TransactionInstruction[];
}
