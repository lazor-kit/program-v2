import * as anchor from '@coral-xyz/anchor';
import { DefaultRule } from '../target/types/default_rule';
import * as constants from './constants';

export class DefaultRuleProgram {
  private connection: anchor.web3.Connection;
  private Idl: anchor.Idl = require('../target/idl/default_rule.json');

  constructor(connection: anchor.web3.Connection) {
    this.connection = connection;
  }

  get program(): anchor.Program<DefaultRule> {
    return new anchor.Program(this.Idl, {
      connection: this.connection,
    });
  }

  get programId(): anchor.web3.PublicKey {
    return this.program.programId;
  }

  rule(smartWalletAuthenticator: anchor.web3.PublicKey): anchor.web3.PublicKey {
    return anchor.web3.PublicKey.findProgramAddressSync(
      [constants.RULE_SEED, smartWalletAuthenticator.toBuffer()],
      this.programId
    )[0];
  }

  async initRuleIns(
    payer: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    smartWalletAuthenticator: anchor.web3.PublicKey
  ) {
    return await this.program.methods
      .initRule()
      .accountsPartial({
        payer,
        smartWallet,
        smartWalletAuthenticator,
        rule: this.rule(smartWalletAuthenticator),
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .instruction();
  }

  async checkRuleIns(
    smartWallet: anchor.web3.PublicKey,
    smartWalletAuthenticator: anchor.web3.PublicKey
  ) {
    return await this.program.methods
      .checkRule()
      .accountsPartial({
        rule: this.rule(smartWallet),
        smartWalletAuthenticator,
      })
      .instruction();
  }

  async addDeviceIns(
    payer: anchor.web3.PublicKey,
    smartWalletAuthenticator: anchor.web3.PublicKey,
    newSmartWalletAuthenticator: anchor.web3.PublicKey
  ) {
    return await this.program.methods
      .addDevice()
      .accountsPartial({
        payer,
        smartWalletAuthenticator,
        newSmartWalletAuthenticator,
        rule: this.rule(smartWalletAuthenticator),
        newRule: this.rule(newSmartWalletAuthenticator),
      })
      .instruction();
  }
}
