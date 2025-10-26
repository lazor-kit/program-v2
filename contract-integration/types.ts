import * as anchor from '@coral-xyz/anchor';
import { Transaction, VersionedTransaction } from '@solana/web3.js';
import { Lazorkit } from './anchor/types/lazorkit';

// ============================================================================
// Core Types (from on-chain)
// ============================================================================
export type WalletState = anchor.IdlTypes<Lazorkit>['walletState'];
export type WalletDevice = anchor.IdlTypes<Lazorkit>['walletDevice'];
export type ProgramConfig = anchor.IdlTypes<Lazorkit>['config'];
export type PolicyProgramRegistry =
  anchor.IdlTypes<Lazorkit>['policyProgramRegistry'];
export type Chunk = anchor.IdlTypes<Lazorkit>['chunk'];

// Instruction Args
export type CreateSmartWalletArgs =
  anchor.IdlTypes<Lazorkit>['createSmartWalletArgs'];
export type ExecuteArgs = anchor.IdlTypes<Lazorkit>['executeArgs'];
export type ChangePolicyArgs = anchor.IdlTypes<Lazorkit>['changePolicyArgs'];
export type CallPolicyArgs = anchor.IdlTypes<Lazorkit>['callPolicyArgs'];
export type CreateChunkArgs = anchor.IdlTypes<Lazorkit>['createChunkArgs'];
export type NewWalletDeviceArgs =
  anchor.IdlTypes<Lazorkit>['newWalletDeviceArgs'];
export type UpdateType = anchor.IdlTypes<Lazorkit>['updateType'];

// ============================================================================
// Smart Wallet Actions
// ============================================================================
export enum SmartWalletAction {
  Execute = 'execute',
  CallPolicy = 'call_policy',
  ChangePolicy = 'change_policy',
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
    newWalletDevice: NewPasskeyDevice | null;
  };
  [SmartWalletAction.ChangePolicy]: {
    destroyPolicyIns: anchor.web3.TransactionInstruction;
    initPolicyIns: anchor.web3.TransactionInstruction;
    newWalletDevice: NewPasskeyDevice | null;
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

export type SmartWalletActionArgs<
  K extends SmartWalletAction = SmartWalletAction
> = {
  type: K;
  args: ArgsByAction[K];
};

// ============================================================================
// Authentication & Transaction Types
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

export interface TransactionBuilderOptions {
  useVersionedTransaction?: boolean;
  addressLookupTable?: anchor.web3.AddressLookupTableAccount;
  recentBlockhash?: string;
  computeUnitLimit?: number;
}

export interface TransactionBuilderResult {
  transaction: Transaction | VersionedTransaction;
  isVersioned: boolean;
  recentBlockhash: string;
}

// ============================================================================
// Base Parameter Types
// ============================================================================
interface BaseParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  vaultIndex?: number;
}

interface AuthParams extends BaseParams {
  passkeySignature: PasskeySignature;
  credentialHash: number[];
}

// ============================================================================
// Parameter Types
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
  amount: anchor.BN;
  policyInstruction?: anchor.web3.TransactionInstruction | null;
  smartWalletId?: anchor.BN;
  referralAddress?: anchor.web3.PublicKey | null;
  vaultIndex?: number;
  policyDataSize?: number;
}

export interface ExecuteParams extends AuthParams {
  policyInstruction: anchor.web3.TransactionInstruction | null;
  cpiInstruction: anchor.web3.TransactionInstruction;
  timestamp: anchor.BN;
  smartWalletId: anchor.BN;
}

export interface CallPolicyParams extends AuthParams {
  policyInstruction: anchor.web3.TransactionInstruction;
  timestamp: anchor.BN;
  newWalletDevice?: NewPasskeyDevice | null;
}

export interface ChangePolicyParams extends AuthParams {
  destroyPolicyInstruction: anchor.web3.TransactionInstruction;
  initPolicyInstruction: anchor.web3.TransactionInstruction;
  newWalletDevice?: NewPasskeyDevice | null;
}

export interface CreateChunkParams extends AuthParams {
  policyInstruction: anchor.web3.TransactionInstruction | null;
  cpiInstructions: anchor.web3.TransactionInstruction[];
  timestamp: anchor.BN;
}

export interface ExecuteChunkParams extends BaseParams {
  cpiInstructions: anchor.web3.TransactionInstruction[];
}

export interface CloseChunkParams extends BaseParams {
  nonce: anchor.BN;
}

export interface GrantPermissionParams extends AuthParams {
  ephemeral_public_key: anchor.web3.PublicKey;
  expiresAt: number;
  cpiInstructions: anchor.web3.TransactionInstruction[];
}

export interface ExecuteWithPermissionParams {
  feePayer: anchor.web3.PublicKey;
  ephemeralSigner: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  permission: anchor.web3.PublicKey;
  cpiInstructions: anchor.web3.TransactionInstruction[];
}
