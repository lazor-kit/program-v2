import * as anchor from '@coral-xyz/anchor';
import { Lazorkit } from '../target/types/lazorkit';

// Account types
export type SmartWalletConfig = anchor.IdlTypes<Lazorkit>['smartWalletConfig'];
export type SmartWalletAuthenticator =
  anchor.IdlTypes<Lazorkit>['smartWalletAuthenticator'];
export type Config = anchor.IdlTypes<Lazorkit>['config'];
export type WhitelistRulePrograms =
  anchor.IdlTypes<Lazorkit>['whitelistRulePrograms'];

// argument type
export type CreatwSmartWalletArgs =
  anchor.IdlTypes<Lazorkit>['creatwSmartWalletArgs'];
export type ExecuteTxnArgs = anchor.IdlTypes<Lazorkit>['executeTxnArgs'];
export type ChangeRuleArgs = anchor.IdlTypes<Lazorkit>['changeRuleArgs'];
export type CallRuleArgs = anchor.IdlTypes<Lazorkit>['callRuleArgs'];
export type CommitArgs = anchor.IdlTypes<Lazorkit>['commitArgs'];
export type NewAuthenticatorArgs =
  anchor.IdlTypes<Lazorkit>['newAuthenticatorArgs'];

// Enum types
export type UpdateConfigType = anchor.IdlTypes<Lazorkit>['updateConfigType'];
