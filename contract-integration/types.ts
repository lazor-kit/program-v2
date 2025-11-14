import * as anchor from '@coral-xyz/anchor';
import { Lazorkit } from './anchor/types/lazorkit';

// ============================================================================
// Core Types (from on-chain)
// ============================================================================
export type WalletState = anchor.IdlTypes<Lazorkit>['walletState'];
export type WalletDevice = anchor.IdlTypes<Lazorkit>['walletDevice'];
export type Chunk = anchor.IdlTypes<Lazorkit>['chunk'];

// Instruction Args
export type CreateSmartWalletArgs =
  anchor.IdlTypes<Lazorkit>['createSmartWalletArgs'];
export type ExecuteArgs = anchor.IdlTypes<Lazorkit>['executeArgs'];
export type CreateChunkArgs = anchor.IdlTypes<Lazorkit>['createChunkArgs'];

// ============================================================================
// Smart Wallet Actions
// ============================================================================
export enum SmartWalletAction {
  Execute = 'execute',
  CreateChunk = 'create_chunk',
  ExecuteChunk = 'execute_chunk',
}

export type ArgsByAction = {
  [SmartWalletAction.Execute]: {
    policyInstruction: anchor.web3.TransactionInstruction | null;
    cpiInstruction: anchor.web3.TransactionInstruction;
    cpiSigners?: anchor.web3.PublicKey[];
  };
  [SmartWalletAction.CreateChunk]: {
    policyInstruction: anchor.web3.TransactionInstruction | null;
    cpiInstructions: anchor.web3.TransactionInstruction[];
    expiresAt: number;
    cpiSigners?: anchor.web3.PublicKey[];
  };
  [SmartWalletAction.ExecuteChunk]: {
    cpiInstructions: anchor.web3.TransactionInstruction[];
    cpiSigners?: anchor.web3.PublicKey[];
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

export interface TransactionBuilderOptions {
  useVersionedTransaction?: boolean;
  addressLookupTable?: anchor.web3.AddressLookupTableAccount;
  recentBlockhash?: string;
  computeUnitLimit?: number;
}

export interface TransactionBuilderResult {
  transaction: anchor.web3.Transaction | anchor.web3.VersionedTransaction;
  isVersioned: boolean;
  recentBlockhash: string;
}

// ============================================================================
// Base Parameter Types
// ============================================================================
interface BaseParams {
  payer: anchor.web3.PublicKey;
  smartWallet: anchor.web3.PublicKey;
}

interface AuthParams extends BaseParams {
  passkeySignature: PasskeySignature;
  credentialHash: number[];
}

// ============================================================================
// Parameter Types
// ============================================================================

export interface CreateSmartWalletParams {
  payer: anchor.web3.PublicKey;
  passkeyPublicKey: number[];
  credentialIdBase64: string;
  amount?: anchor.BN;
  policyInstruction?: anchor.web3.TransactionInstruction | null;
  smartWalletId?: anchor.BN;
  policyDataSize?: number;
}

export interface ExecuteParams extends AuthParams {
  policyInstruction: anchor.web3.TransactionInstruction | null;
  cpiInstruction: anchor.web3.TransactionInstruction;
  timestamp: anchor.BN;
  smartWalletId: anchor.BN;
  cpiSigners?: anchor.web3.PublicKey[];
}

export interface CreateChunkParams extends AuthParams {
  policyInstruction: anchor.web3.TransactionInstruction | null;
  cpiInstructions: anchor.web3.TransactionInstruction[];
  timestamp: anchor.BN;
  cpiSigners?: anchor.web3.PublicKey[];
}

export interface ExecuteChunkParams extends BaseParams {
  cpiInstructions: anchor.web3.TransactionInstruction[];
  cpiSigners?: anchor.web3.PublicKey[];
}

export interface CloseChunkParams extends BaseParams {
  nonce: anchor.BN;
}
