import * as anchor from "@coral-xyz/anchor";
import {
  Connection,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import DefaultRuleIdl from "../../target/idl/default_rule.json";
import { DefaultRule } from "../../target/types/default_rule";
import { deriveRulePda } from "../pda/defaultRule";
import { decodeAnchorError } from "../errors";

export type DefaultRuleClientOptions = {
  connection: Connection;
  programId?: PublicKey;
};

export class DefaultRuleClient {
  readonly connection: Connection;
  readonly program: anchor.Program<DefaultRule>;
  readonly programId: PublicKey;

  constructor(opts: DefaultRuleClientOptions) {
    this.connection = opts.connection;
    const programDefault = new anchor.Program(DefaultRuleIdl as anchor.Idl, {
      connection: opts.connection,
    }) as unknown as anchor.Program<DefaultRule>;
    this.programId = opts.programId ?? programDefault.programId;
    this.program = new (anchor as any).Program(
      DefaultRuleIdl as anchor.Idl,
      this.programId,
      { connection: opts.connection }
    ) as anchor.Program<DefaultRule>;
  }

  rulePda(smartWalletAuthenticator: PublicKey): PublicKey {
    return deriveRulePda(this.programId, smartWalletAuthenticator);
  }

  async buildInitRuleIx(
    payer: PublicKey,
    smartWallet: PublicKey,
    smartWalletAuthenticator: PublicKey
  ): Promise<TransactionInstruction> {
    try {
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
    } catch (e) {
      throw decodeAnchorError(e);
    }
  }

  async buildCheckRuleIx(
    smartWalletAuthenticator: PublicKey
  ): Promise<TransactionInstruction> {
    try {
      return await this.program.methods
        .checkRule()
        .accountsPartial({
          rule: this.rulePda(smartWalletAuthenticator),
          smartWalletAuthenticator,
        })
        .instruction();
    } catch (e) {
      throw decodeAnchorError(e);
    }
  }

  async buildAddDeviceIx(
    payer: PublicKey,
    smartWalletAuthenticator: PublicKey,
    newSmartWalletAuthenticator: PublicKey
  ): Promise<TransactionInstruction> {
    try {
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
    } catch (e) {
      throw decodeAnchorError(e);
    }
  }
}
