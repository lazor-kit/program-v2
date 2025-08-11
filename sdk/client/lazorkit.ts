import { Program, BN } from '@coral-xyz/anchor';
import {
  PublicKey,
  Transaction,
  TransactionMessage,
  TransactionInstruction,
  Connection,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  VersionedTransaction,
  AccountMeta,
} from '@solana/web3.js';
import LazorkitIdl from '../../target/idl/lazorkit.json';
import { Lazorkit } from '../../target/types/lazorkit';
import {
  deriveConfigPda,
  deriveWhitelistRuleProgramsPda,
  deriveSmartWalletPda,
  deriveSmartWalletConfigPda,
  deriveSmartWalletAuthenticatorPda,
  deriveCpiCommitPda,
} from '../pda/lazorkit';
import { buildSecp256r1VerifyIx } from '../webauthn/secp256r1';
import { instructionToAccountMetas } from '../utils';
import { sha256 } from 'js-sha256';
import * as types from '../types';
import DefaultRuleIdl from '../../target/idl/default_rule.json';
import { DefaultRule } from '../../target/types/default_rule';
import { randomBytes } from 'crypto';

export class LazorkitClient {
  readonly connection: Connection;
  readonly program: Program<Lazorkit>;
  readonly programId: PublicKey;
  readonly defaultRuleProgram: Program<DefaultRule>;

  constructor(connection: Connection) {
    this.connection = connection;

    this.program = new Program<Lazorkit>(LazorkitIdl as Lazorkit, {
      connection: connection,
    });
    this.defaultRuleProgram = new Program<DefaultRule>(
      DefaultRuleIdl as DefaultRule,
      {
        connection: connection,
      }
    );
    this.programId = this.program.programId;
  }

  // PDAs
  configPda(): PublicKey {
    return deriveConfigPda(this.programId);
  }
  whitelistRuleProgramsPda(): PublicKey {
    return deriveWhitelistRuleProgramsPda(this.programId);
  }
  smartWalletPda(walletId: BN): PublicKey {
    return deriveSmartWalletPda(this.programId, walletId);
  }
  smartWalletConfigPda(smartWallet: PublicKey): PublicKey {
    return deriveSmartWalletConfigPda(this.programId, smartWallet);
  }
  smartWalletAuthenticatorPda(
    smartWallet: PublicKey,
    passkey: number[]
  ): PublicKey {
    return deriveSmartWalletAuthenticatorPda(
      this.programId,
      smartWallet,
      passkey
    )[0];
  }
  cpiCommitPda(smartWallet: PublicKey, lastNonce: BN): PublicKey {
    return deriveCpiCommitPda(this.programId, smartWallet, lastNonce);
  }

  // Convenience helpers
  generateWalletId(): BN {
    return new BN(randomBytes(8), 'le');
  }

  async getConfigData() {
    return await this.program.account.config.fetch(this.configPda());
  }
  async getSmartWalletConfigData(smartWallet: PublicKey) {
    const pda = this.smartWalletConfigPda(smartWallet);
    return await this.program.account.smartWalletConfig.fetch(pda);
  }
  async getSmartWalletAuthenticatorData(smartWalletAuthenticator: PublicKey) {
    return await this.program.account.smartWalletAuthenticator.fetch(
      smartWalletAuthenticator
    );
  }

  // Builders (TransactionInstruction)
  async buildInitializeIx(
    payer: PublicKey,
    defaultRuleProgram: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .initialize()
      .accountsPartial({
        signer: payer,
        config: this.configPda(),
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        defaultRuleProgram,
        systemProgram: SystemProgram.programId,
      })
      .instruction();
  }

  async buildCreateSmartWalletIx(
    payer: PublicKey,
    args: types.CreatwSmartWalletArgs,
    ruleInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    const smartWallet = this.smartWalletPda(args.walletId);

    return await this.program.methods
      .createSmartWallet(args)
      .accountsPartial({
        signer: payer,
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(
          smartWallet,
          args.passkeyPubkey
        ),
        config: this.configPda(),
        defaultRuleProgram: this.defaultRuleProgram.programId,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([...instructionToAccountMetas(ruleInstruction, payer)])
      .instruction();
  }

  async buildExecuteTxnDirectIx(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.ExecuteTxnArgs,
    ruleInstruction: TransactionInstruction,
    cpiInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .executeTxnDirect(args)
      .accountsPartial({
        payer,
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(
          smartWallet,
          args.passkeyPubkey
        ),
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        authenticatorProgram: ruleInstruction.programId,
        cpiProgram: cpiInstruction.programId,
        config: this.configPda(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts([
        ...instructionToAccountMetas(ruleInstruction, payer),
        ...instructionToAccountMetas(cpiInstruction, payer),
      ])
      .instruction();
  }

  async buildCallRuleDirectIx(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.CallRuleArgs,
    ruleInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    const remaining: AccountMeta[] = [];

    if (args.newAuthenticator) {
      const newSmartWalletAuthenticator = this.smartWalletAuthenticatorPda(
        smartWallet,
        args.newAuthenticator.passkeyPubkey
      );
      remaining.push({
        pubkey: newSmartWalletAuthenticator,
        isWritable: true,
        isSigner: false,
      });
    }

    remaining.push(...instructionToAccountMetas(ruleInstruction, payer));

    return await this.program.methods
      .callRuleDirect(args)
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(
          smartWallet,
          args.passkeyPubkey
        ),
        ruleProgram: ruleInstruction.programId,
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  async buildChangeRuleDirectIx(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.ChangeRuleArgs,
    destroyRuleInstruction: TransactionInstruction,
    initRuleInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .changeRuleDirect(args)
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(
          smartWallet,
          args.passkeyPubkey
        ),
        oldRuleProgram: destroyRuleInstruction.programId,
        newRuleProgram: initRuleInstruction.programId,
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([
        ...instructionToAccountMetas(destroyRuleInstruction, payer),
        ...instructionToAccountMetas(initRuleInstruction, payer),
      ])
      .instruction();
  }

  async buildCommitCpiIx(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.CommitArgs,
    ruleInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .commitCpi(args)
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(
          smartWallet,
          args.passkeyPubkey
        ),
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        authenticatorProgram: ruleInstruction.programId,
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([...instructionToAccountMetas(ruleInstruction, payer)])
      .instruction();
  }

  async buildExecuteCommittedIx(
    payer: PublicKey,
    smartWallet: PublicKey,
    cpiInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const cpiCommit = this.cpiCommitPda(smartWallet, cfg.lastNonce);

    return await this.program.methods
      .executeCommitted(cpiInstruction.data)
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        cpiProgram: cpiInstruction.programId,
        cpiCommit,
        commitRefund: payer,
      })
      .remainingAccounts([...instructionToAccountMetas(cpiInstruction, payer)])
      .instruction();
  }

  // High-level: build transactions with Secp256r1 verify ix at index 0
  async executeTxnDirectTx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    ruleInstruction: TransactionInstruction;
    cpiInstruction: TransactionInstruction;
    ruleProgram?: PublicKey;
  }): Promise<VersionedTransaction> {
    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([
        Buffer.from(params.authenticatorDataRaw),
        Buffer.from(sha256.hex(params.clientDataJsonRaw), 'hex'),
      ]),
      Buffer.from(params.passkey33),
      Buffer.from(params.signature64)
    );

    const execIx = await this.buildExecuteTxnDirectIx(
      params.payer,
      params.smartWallet,
      {
        passkeyPubkey: Array.from(params.passkey33),
        signature: Buffer.from(params.signature64),
        clientDataJsonRaw: Buffer.from(params.clientDataJsonRaw),
        authenticatorDataRaw: Buffer.from(params.authenticatorDataRaw),
        verifyInstructionIndex: 0,
        ruleData: params.ruleInstruction.data,
        cpiData: params.cpiInstruction.data,
        splitIndex: params.ruleInstruction.keys.length,
      },
      params.ruleInstruction,
      params.cpiInstruction
    );
    return this.buildV0Tx(params.payer, [verifyIx, execIx]);
  }

  async callRuleDirectTx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    ruleProgram: PublicKey;
    ruleInstruction: TransactionInstruction;
    newAuthenticator?: {
      passkey33: Uint8Array;
      credentialIdBase64: string;
    }; // optional
  }): Promise<VersionedTransaction> {
    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([
        Buffer.from(params.authenticatorDataRaw),
        Buffer.from(sha256.hex(params.clientDataJsonRaw), 'hex'),
      ]),
      Buffer.from(params.passkey33),
      Buffer.from(params.signature64)
    );
    const ix = await this.buildCallRuleDirectIx(
      params.payer,
      params.smartWallet,
      {
        passkeyPubkey: Array.from(params.passkey33),
        signature: Buffer.from(params.signature64),
        clientDataJsonRaw: Buffer.from(params.clientDataJsonRaw),
        authenticatorDataRaw: Buffer.from(params.authenticatorDataRaw),
        newAuthenticator: params.newAuthenticator
          ? {
              passkeyPubkey: Array.from(params.newAuthenticator.passkey33),
              credentialId: Buffer.from(
                params.newAuthenticator.credentialIdBase64,
                'base64'
              ),
            }
          : null,
        ruleData: params.ruleInstruction.data,
        verifyInstructionIndex:
          (params.newAuthenticator ? 1 : 0) +
          params.ruleInstruction.keys.length,
      },
      params.ruleInstruction
    );
    return this.buildV0Tx(params.payer, [verifyIx, ix]);
  }

  async changeRuleDirectTx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    destroyRuleInstruction: TransactionInstruction;
    initRuleInstruction: TransactionInstruction;
    newAuthenticator?: {
      passkey33: Uint8Array;
      credentialIdBase64: string;
    }; // optional
  }): Promise<VersionedTransaction> {
    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([
        Buffer.from(params.authenticatorDataRaw),
        Buffer.from(sha256.hex(params.clientDataJsonRaw), 'hex'),
      ]),
      Buffer.from(params.passkey33),
      Buffer.from(params.signature64)
    );

    const ix = await this.buildChangeRuleDirectIx(
      params.payer,
      params.smartWallet,
      {
        passkeyPubkey: Array.from(params.passkey33),
        signature: Buffer.from(params.signature64),
        clientDataJsonRaw: Buffer.from(params.clientDataJsonRaw),
        authenticatorDataRaw: Buffer.from(params.authenticatorDataRaw),
        verifyInstructionIndex: 0,
        destroyRuleData: params.destroyRuleInstruction.data,
        initRuleData: params.initRuleInstruction.data,
        splitIndex:
          (params.newAuthenticator ? 1 : 0) +
          params.destroyRuleInstruction.keys.length,
        newAuthenticator: params.newAuthenticator
          ? {
              passkeyPubkey: Array.from(params.newAuthenticator.passkey33),
              credentialId: Buffer.from(
                params.newAuthenticator.credentialIdBase64,
                'base64'
              ),
            }
          : null,
      },
      params.destroyRuleInstruction,
      params.initRuleInstruction
    );
    return this.buildV0Tx(params.payer, [verifyIx, ix]);
  }

  async commitCpiTx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    ruleInstruction: TransactionInstruction;
    cpiProgram: PublicKey;
    expiresAt: number;
  }) {
    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([
        Buffer.from(params.authenticatorDataRaw),
        Buffer.from(sha256.hex(params.clientDataJsonRaw), 'hex'),
      ]),
      Buffer.from(params.passkey33),
      Buffer.from(params.signature64)
    );
    const ix = await this.buildCommitCpiIx(
      params.payer,
      params.smartWallet,
      {
        passkeyPubkey: Array.from(params.passkey33),
        signature: Buffer.from(params.signature64),
        clientDataJsonRaw: Buffer.from(params.clientDataJsonRaw),
        authenticatorDataRaw: Buffer.from(params.authenticatorDataRaw),
        expiresAt: new BN(params.expiresAt),
        ruleData: params.ruleInstruction.data,
        verifyInstructionIndex: 0,
      },
      params.ruleInstruction
    );
    const tx = new Transaction().add(verifyIx).add(ix);
    tx.feePayer = params.payer;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
    return tx;
  }

  // Convenience: VersionedTransaction v0
  async buildV0Tx(
    payer: PublicKey,
    ixs: TransactionInstruction[]
  ): Promise<VersionedTransaction> {
    const { blockhash } = await this.connection.getLatestBlockhash();
    const msg = new TransactionMessage({
      payerKey: payer,
      recentBlockhash: blockhash,
      instructions: ixs,
    }).compileToV0Message();
    return new VersionedTransaction(msg);
  }

  // Legacy-compat APIs for simpler DX
  async initializeTxn(payer: PublicKey, defaultRuleProgram: PublicKey) {
    const ix = await this.buildInitializeIx(payer, defaultRuleProgram);
    return new Transaction().add(ix);
  }

  async createSmartWalletTx(params: {
    payer: PublicKey;
    passkey33: Uint8Array;
    credentialIdBase64: string;
    ruleInstruction: TransactionInstruction;
    isPayForUser?: boolean;
    defaultRuleProgram: PublicKey;
    smartWalletId?: BN;
  }) {
    let smartWalletId: BN = this.generateWalletId();
    if (params.smartWalletId) {
      smartWalletId = params.smartWalletId;
    }
    const args = {
      passkeyPubkey: Array.from(params.passkey33),
      credentialId: Buffer.from(params.credentialIdBase64, 'base64'),
      ruleData: params.ruleInstruction.data,
      walletId: smartWalletId,
      isPayForUser: params.isPayForUser,
    };
    const ix = await this.buildCreateSmartWalletIx(
      params.payer,
      args,
      params.ruleInstruction
    );
    const tx = new Transaction().add(ix);
    tx.feePayer = params.payer;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
    const smartWallet = this.smartWalletPda(smartWalletId);
    return {
      transaction: tx,
      smartWalletId: smartWalletId,
      smartWallet,
    };
  }
}
