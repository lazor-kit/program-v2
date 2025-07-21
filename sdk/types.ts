import * as anchor from '@coral-xyz/anchor';

import { Lazorkit } from '../target/types/lazorkit';

export type SmartWalletSeq = anchor.IdlTypes<Lazorkit>['smartWalletSeq'];
export type SmartWalletConfig = anchor.IdlTypes<Lazorkit>['smartWalletConfig'];
export type SmartWalletAuthenticator =
  anchor.IdlTypes<Lazorkit>['smartWalletAuthenticator'];

export const ExecuteAction = {
  ExecuteTx: { executeTx: {} },
  ChangeRuleProgram: { changeRuleProgram: {} },
  CallRuleProgram: { callRuleProgram: {} },
};

export type ExecuteActionType = anchor.IdlTypes<Lazorkit>['action'];
