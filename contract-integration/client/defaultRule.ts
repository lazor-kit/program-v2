import * as anchor from '@coral-xyz/anchor';
import { Connection, PublicKey, SystemProgram, TransactionInstruction } from '@solana/web3.js';
import DefaultRuleIdl from '../anchor/idl/default_rule.json';
import { DefaultRule } from '../anchor/types/default_rule';
import { deriveRulePda } from '../pda/defaultRule';

export class DefaultRuleClient {
  readonly connection: Connection;
  readonly program: anchor.Program<DefaultRule>;
  readonly programId: PublicKey;

  constructor(connection: Connection) {
    this.connection = connection;

    this.program = new anchor.Program<DefaultRule>(DefaultRuleIdl as DefaultRule, {
      connection: connection,
    });
    this.programId = this.program.programId;
  }

  rulePda(smartWalletAuthenticator: PublicKey): PublicKey {
    return deriveRulePda(this.programId, smartWalletAuthenticator);
  }

  async buildInitRuleIx(
    payer: PublicKey,
    smartWallet: PublicKey,
    smartWalletAuthenticator: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .initRule()
      .accountsPartial({
        payer,
        smartWallet,
        smartWalletAuthenticator,
        rule: this.rulePda(smartWalletAuthenticator),
        systemProgram: SystemProgram.programId,
      })
      .instruction();
  }

  async buildCheckRuleIx(smartWalletAuthenticator: PublicKey): Promise<TransactionInstruction> {
    return await this.program.methods
      .checkRule()
      .accountsPartial({
        rule: this.rulePda(smartWalletAuthenticator),
        smartWalletAuthenticator,
      })
      .instruction();
  }

  async buildAddDeviceIx(
    payer: PublicKey,
    smartWalletAuthenticator: PublicKey,
    newSmartWalletAuthenticator: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .addDevice()
      .accountsPartial({
        payer,
        smartWalletAuthenticator,
        newSmartWalletAuthenticator,
        rule: this.rulePda(smartWalletAuthenticator),
        newRule: this.rulePda(newSmartWalletAuthenticator),
        systemProgram: SystemProgram.programId,
      })
      .instruction();
  }
}
