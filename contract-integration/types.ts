import * as anchor from '@coral-xyz/anchor';
import { Lazorkit } from './anchor/types/lazorkit';

// Account types
export type SmartWalletConfig = anchor.IdlTypes<Lazorkit>['smartWalletConfig'];
export type SmartWalletAuthenticator = anchor.IdlTypes<Lazorkit>['smartWalletAuthenticator'];
export type Config = anchor.IdlTypes<Lazorkit>['config'];
export type WhitelistRulePrograms = anchor.IdlTypes<Lazorkit>['whitelistRulePrograms'];

// argument type
export type CreatwSmartWalletArgs = anchor.IdlTypes<Lazorkit>['creatwSmartWalletArgs'];
export type ExecuteTxnArgs = anchor.IdlTypes<Lazorkit>['executeTxnArgs'];
export type ChangeRuleArgs = anchor.IdlTypes<Lazorkit>['changeRuleArgs'];
export type CallRuleArgs = anchor.IdlTypes<Lazorkit>['callRuleArgs'];
export type CommitArgs = anchor.IdlTypes<Lazorkit>['commitArgs'];
export type NewAuthenticatorArgs = anchor.IdlTypes<Lazorkit>['newAuthenticatorArgs'];

// Enum types
export type UpdateConfigType = anchor.IdlTypes<Lazorkit>['updateConfigType'];

export enum SmartWalletAction {
  ChangeRule,
  CallRule,
  ExecuteTx,
}

export type ArgsByAction = {
  [SmartWalletAction.ExecuteTx]: {
    ruleInstruction: anchor.web3.TransactionInstruction | null;
    cpiInstruction: anchor.web3.TransactionInstruction;
  };
  [SmartWalletAction.CallRule]: {
    ruleInstruction: anchor.web3.TransactionInstruction;
    newPasskey: number[];
  };
  [SmartWalletAction.ChangeRule]: {
    destroyRuleIns: anchor.web3.TransactionInstruction;
    initRuleIns: anchor.web3.TransactionInstruction;
    newPasskey: number[];
  };
};

export type MessageArgs<K extends SmartWalletAction = SmartWalletAction> = {
  type: K;
  args: ArgsByAction[K];
};
