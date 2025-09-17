import * as anchor from '@coral-xyz/anchor';
import { Lazorkit } from './anchor/types/lazorkit';

// ============================================================================
// Account Types (from on-chain state)
// ============================================================================
export type SmartWalletData = anchor.IdlTypes<Lazorkit>['smartWalletData'];
export type WalletDevice = anchor.IdlTypes<Lazorkit>['walletDevice'];
export type ProgramConfig = anchor.IdlTypes<Lazorkit>['programConfig'];
export type PolicyProgramRegistry =
  anchor.IdlTypes<Lazorkit>['policyProgramRegistry'];
export type TransactionSession =
  anchor.IdlTypes<Lazorkit>['transactionSession'];
export type EphemeralAuthorization =
  anchor.IdlTypes<Lazorkit>['ephemeralAuthorization'];

// ============================================================================
// Instruction Argument Types (from on-chain instructions)
// ============================================================================
export type CreateSmartWalletArgs =
  anchor.IdlTypes<Lazorkit>['createSmartWalletArgs'];
export type ExecuteDirectTransactionArgs =
  anchor.IdlTypes<Lazorkit>['executeDirectTransactionArgs'];
export type UpdateWalletPolicyArgs =
  anchor.IdlTypes<Lazorkit>['updateWalletPolicyArgs'];
export type InvokeWalletPolicyArgs =
  anchor.IdlTypes<Lazorkit>['invokeWalletPolicyArgs'];
export type CreateDeferredExecutionArgs =
  anchor.IdlTypes<Lazorkit>['createDeferredExecutionArgs'];
export type AuthorizeEphemeralExecutionArgs =
  anchor.IdlTypes<Lazorkit>['authorizeEphemeralExecutionArgs'];
export type ExecuteEphemeralAuthorizationArgs =
  anchor.IdlTypes<Lazorkit>['authorizeEphemeralExecutionArgs'];
export type NewWalletDeviceArgs =
  anchor.IdlTypes<Lazorkit>['newWalletDeviceArgs'];

// ============================================================================
// Configuration Types
// ============================================================================
export type UpdateType = anchor.IdlTypes<Lazorkit>['configUpdateType'];

// ============================================================================
// Smart Wallet Action Types
// ============================================================================
export enum SmartWalletAction {
  UpdateWalletPolicy = 'update_wallet_policy',
  InvokeWalletPolicy = 'invoke_wallet_policy',
  ExecuteDirectTransaction = 'execute_direct_transaction',
  CreateDeferredExecution = 'create_deferred_execution',
  ExecuteDeferredTransaction = 'execute_deferred_transaction',
  AuthorizeEphemeralExecution = 'authorize_ephemeral_execution',
  ExecuteEphemeralAuthorization = 'execute_ephemeral_authorization',
}

export type ArgsByAction = {
  [SmartWalletAction.ExecuteDirectTransaction]: {
    policyInstruction: anchor.web3.TransactionInstruction | null;
    cpiInstruction: anchor.web3.TransactionInstruction;
  };
  [SmartWalletAction.InvokeWalletPolicy]: {
    policyInstruction: anchor.web3.TransactionInstruction;
    newWalletDevice: {
      passkeyPublicKey: number[];
      credentialIdBase64: string;
    } | null;
  };
  [SmartWalletAction.UpdateWalletPolicy]: {
    destroyPolicyIns: anchor.web3.TransactionInstruction;
    initPolicyIns: anchor.web3.TransactionInstruction;
    newWalletDevice: {
      passkeyPublicKey: number[];
      credentialIdBase64: string;
    } | null;
  };
  [SmartWalletAction.CreateDeferredExecution]: {
    policyInstruction: anchor.web3.TransactionInstruction | null;
    expiresAt: number;
  };
  [SmartWalletAction.ExecuteDeferredTransaction]: {
    cpiInstructions: anchor.web3.TransactionInstruction[];
  };
  [SmartWalletAction.AuthorizeEphemeralExecution]: {
    ephemeral_public_key: anchor.web3.PublicKey;
    expiresAt: number;
    cpiInstructions: anchor.web3.TransactionInstruction[];
  };
  [SmartWalletAction.ExecuteEphemeralAuthorization]: {
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

export interface ExecuteDirectTransactionParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  policyInstruction: anchor.web3.TransactionInstruction | null;
  cpiInstruction: anchor.web3.TransactionInstruction;
  vaultIndex?: number;
}

export interface InvokeWalletPolicyParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  policyInstruction: anchor.web3.TransactionInstruction;
  newWalletDevice?: NewPasskeyDevice | null;
  vaultIndex?: number;
}

export interface UpdateWalletPolicyParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  destroyPolicyInstruction: anchor.web3.TransactionInstruction;
  initPolicyInstruction: anchor.web3.TransactionInstruction;
  newWalletDevice?: NewPasskeyDevice | null;
  vaultIndex?: number;
}

export interface CreateDeferredExecutionParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  policyInstruction: anchor.web3.TransactionInstruction | null;
  expiresAt: number;
  vaultIndex?: number;
}

export interface ExecuteDeferredTransactionParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  cpiInstructions: anchor.web3.TransactionInstruction[];
}

export interface AuthorizeEphemeralExecutionParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  passkeySignature: PasskeySignature;
  ephemeral_public_key: anchor.web3.PublicKey;
  expiresAt: number;
  cpiInstructions: anchor.web3.TransactionInstruction[];
  vaultIndex?: number;
}

export interface ExecuteEphemeralAuthorizationParams {
  feePayer: anchor.web3.PublicKey;
  ephemeralSigner: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
  ephemeralAuthorization: anchor.web3.PublicKey;
  cpiInstructions: anchor.web3.TransactionInstruction[];
}
