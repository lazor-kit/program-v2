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
import LazorkitIdl from '../anchor/idl/lazorkit.json';
import { Lazorkit } from '../anchor/types/lazorkit';
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
import { randomBytes } from 'crypto';
import { DefaultRuleClient } from './defaultRule';
import * as bs58 from 'bs58';
import { Buffer } from 'buffer';
import { buildCallRuleMessage, buildChangeRuleMessage, buildExecuteMessage } from '../messages';

export class LazorkitClient {
  readonly connection: Connection;
  readonly program: Program<Lazorkit>;
  readonly programId: PublicKey;
  readonly defaultRuleProgram: DefaultRuleClient;

  constructor(connection: Connection) {
    this.connection = connection;

    this.program = new Program<Lazorkit>(LazorkitIdl as Lazorkit, {
      connection: connection,
    });
    this.defaultRuleProgram = new DefaultRuleClient(connection);
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
  smartWalletAuthenticatorPda(smartWallet: PublicKey, passkey: number[]): PublicKey {
    return deriveSmartWalletAuthenticatorPda(this.programId, smartWallet, passkey)[0];
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
    return await this.program.account.smartWalletAuthenticator.fetch(smartWalletAuthenticator);
  }
  async getSmartWalletByPasskey(passkeyPubkey: number[]): Promise<{
    smartWallet: PublicKey | null;
    smartWalletAuthenticator: PublicKey | null;
  }> {
    const discriminator = LazorkitIdl.accounts.find(
      (a: any) => a.name === 'SmartWalletAuthenticator'
    )!.discriminator;

    const accounts = await this.connection.getProgramAccounts(this.programId, {
      dataSlice: {
        offset: 8,
        length: 33,
      },
      filters: [
        { memcmp: { offset: 0, bytes: bs58.encode(discriminator) } },
        { memcmp: { offset: 8, bytes: bs58.encode(passkeyPubkey) } },
      ],
    });

    if (accounts.length === 0) {
      return { smartWalletAuthenticator: null, smartWallet: null };
    }

    const smartWalletAuthenticatorData = await this.getSmartWalletAuthenticatorData(
      accounts[0].pubkey
    );

    return {
      smartWalletAuthenticator: accounts[0].pubkey,
      smartWallet: smartWalletAuthenticatorData.smartWallet,
    };
  }

  // Builders (TransactionInstruction)
  async buildInitializeIx(payer: PublicKey): Promise<TransactionInstruction> {
    return await this.program.methods
      .initialize()
      .accountsPartial({
        signer: payer,
        config: this.configPda(),
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        defaultRuleProgram: this.defaultRuleProgram.programId,
        systemProgram: SystemProgram.programId,
      })
      .instruction();
  }

  async buildCreateSmartWalletIx(
    payer: PublicKey,
    smartWallet: PublicKey,
    smartWalletAuthenticator: PublicKey,
    ruleInstruction: TransactionInstruction,
    args: types.CreatwSmartWalletArgs
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .createSmartWallet(args)
      .accountsPartial({
        signer: payer,
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        smartWalletAuthenticator,
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
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(smartWallet, args.passkeyPubkey),
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
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(smartWallet, args.passkeyPubkey),
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

    remaining.push(...instructionToAccountMetas(destroyRuleInstruction, payer));
    remaining.push(...instructionToAccountMetas(initRuleInstruction, payer));

    return await this.program.methods
      .changeRuleDirect(args)
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(smartWallet, args.passkeyPubkey),
        oldRuleProgram: destroyRuleInstruction.programId,
        newRuleProgram: initRuleInstruction.programId,
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
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
        smartWalletAuthenticator: this.smartWalletAuthenticatorPda(smartWallet, args.passkeyPubkey),
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
    passkeyPubkey: number[];
    signature64: String;
    clientDataJsonRaw64: String;
    authenticatorDataRaw64: String;
    ruleInstruction?: TransactionInstruction;
    cpiInstruction: TransactionInstruction;
  }): Promise<VersionedTransaction> {
    const authenticatorDataRaw = Buffer.from(params.authenticatorDataRaw64, 'base64');
    const clientDataJsonRaw = Buffer.from(params.clientDataJsonRaw64, 'base64');
    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([authenticatorDataRaw, Buffer.from(sha256.hex(clientDataJsonRaw), 'hex')]),
      Buffer.from(params.passkeyPubkey),
      Buffer.from(params.signature64)
    );

    let ruleInstruction = await this.defaultRuleProgram.buildCheckRuleIx(
      this.smartWalletAuthenticatorPda(params.smartWallet, params.passkeyPubkey)
    );
    if (params.ruleInstruction) {
      ruleInstruction = ruleInstruction;
    }

    const execIx = await this.buildExecuteTxnDirectIx(
      params.payer,
      params.smartWallet,
      {
        passkeyPubkey: params.passkeyPubkey,
        signature: Buffer.from(params.signature64, 'base64'),
        clientDataJsonRaw,
        authenticatorDataRaw,
        verifyInstructionIndex: 0,
        ruleData: ruleInstruction.data,
        cpiData: params.cpiInstruction.data,
        splitIndex: ruleInstruction.keys.length,
      },
      ruleInstruction,
      params.cpiInstruction
    );
    return this.buildV0Tx(params.payer, [verifyIx, execIx]);
  }

  async callRuleDirectTx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkeyPubkey: number[];
    signature64: String;
    clientDataJsonRaw64: String;
    authenticatorDataRaw64: String;
    ruleProgram: PublicKey;
    ruleInstruction: TransactionInstruction;
    newAuthenticator?: {
      passkeyPubkey: number[];
      credentialIdBase64: string;
    }; // optional
  }): Promise<VersionedTransaction> {
    const authenticatorDataRaw = Buffer.from(params.authenticatorDataRaw64, 'base64');
    const clientDataJsonRaw = Buffer.from(params.clientDataJsonRaw64, 'base64');

    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([authenticatorDataRaw, Buffer.from(sha256.hex(clientDataJsonRaw), 'hex')]),
      Buffer.from(params.passkeyPubkey),
      Buffer.from(params.signature64)
    );

    const ix = await this.buildCallRuleDirectIx(
      params.payer,
      params.smartWallet,
      {
        passkeyPubkey: params.passkeyPubkey,
        signature: Buffer.from(params.signature64, 'base64'),
        clientDataJsonRaw: clientDataJsonRaw,
        authenticatorDataRaw: authenticatorDataRaw,
        newAuthenticator: params.newAuthenticator
          ? {
              passkeyPubkey: Array.from(params.newAuthenticator.passkeyPubkey),
              credentialId: Buffer.from(params.newAuthenticator.credentialIdBase64, 'base64'),
            }
          : null,
        ruleData: params.ruleInstruction.data,
        verifyInstructionIndex:
          (params.newAuthenticator ? 1 : 0) + params.ruleInstruction.keys.length,
      },
      params.ruleInstruction
    );
    return this.buildV0Tx(params.payer, [verifyIx, ix]);
  }

  async changeRuleDirectTx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkeyPubkey: number[];
    signature64: String;
    clientDataJsonRaw64: String;
    authenticatorDataRaw64: String;
    destroyRuleInstruction: TransactionInstruction;
    initRuleInstruction: TransactionInstruction;
    newAuthenticator?: {
      passkeyPubkey: number[];
      credentialIdBase64: string;
    }; // optional
  }): Promise<VersionedTransaction> {
    const authenticatorDataRaw = Buffer.from(params.authenticatorDataRaw64, 'base64');
    const clientDataJsonRaw = Buffer.from(params.clientDataJsonRaw64, 'base64');

    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([authenticatorDataRaw, Buffer.from(sha256.hex(clientDataJsonRaw), 'hex')]),
      Buffer.from(params.passkeyPubkey),
      Buffer.from(params.signature64)
    );

    const ix = await this.buildChangeRuleDirectIx(
      params.payer,
      params.smartWallet,
      {
        passkeyPubkey: params.passkeyPubkey,
        signature: Buffer.from(params.signature64, 'base64'),
        clientDataJsonRaw,
        authenticatorDataRaw,
        verifyInstructionIndex: 0,
        destroyRuleData: params.destroyRuleInstruction.data,
        initRuleData: params.initRuleInstruction.data,
        splitIndex: (params.newAuthenticator ? 1 : 0) + params.destroyRuleInstruction.keys.length,
        newAuthenticator: params.newAuthenticator
          ? {
              passkeyPubkey: Array.from(params.newAuthenticator.passkeyPubkey),
              credentialId: Buffer.from(params.newAuthenticator.credentialIdBase64, 'base64'),
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
    passkeyPubkey: number[];
    signature64: String;
    clientDataJsonRaw64: String;
    authenticatorDataRaw64: String;
    ruleInstruction?: TransactionInstruction;
    expiresAt: number;
  }) {
    const authenticatorDataRaw = Buffer.from(params.authenticatorDataRaw64, 'base64');
    const clientDataJsonRaw = Buffer.from(params.clientDataJsonRaw64, 'base64');

    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([authenticatorDataRaw, Buffer.from(sha256.hex(clientDataJsonRaw), 'hex')]),
      Buffer.from(params.passkeyPubkey),
      Buffer.from(params.signature64)
    );

    let ruleInstruction = await this.defaultRuleProgram.buildCheckRuleIx(
      this.smartWalletAuthenticatorPda(params.smartWallet, params.passkeyPubkey)
    );
    if (params.ruleInstruction) {
      ruleInstruction = ruleInstruction;
    }

    const ix = await this.buildCommitCpiIx(
      params.payer,
      params.smartWallet,
      {
        passkeyPubkey: params.passkeyPubkey,
        signature: Buffer.from(params.signature64, 'base64'),
        clientDataJsonRaw: clientDataJsonRaw,
        authenticatorDataRaw: authenticatorDataRaw,
        expiresAt: new BN(params.expiresAt),
        ruleData: ruleInstruction.data,
        verifyInstructionIndex: 0,
      },
      ruleInstruction
    );
    return this.buildV0Tx(params.payer, [verifyIx, ix]);
  }

  async executeCommitedTx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    cpiInstruction: TransactionInstruction;
  }): Promise<VersionedTransaction> {
    const ix = await this.buildExecuteCommittedIx(
      params.payer,
      params.smartWallet,
      params.cpiInstruction
    );

    return this.buildV0Tx(params.payer, [ix]);
  }

  // Convenience: VersionedTransaction v0
  async buildV0Tx(payer: PublicKey, ixs: TransactionInstruction[]): Promise<VersionedTransaction> {
    const { blockhash } = await this.connection.getLatestBlockhash();
    const msg = new TransactionMessage({
      payerKey: payer,
      recentBlockhash: blockhash,
      instructions: ixs,
    }).compileToV0Message();
    return new VersionedTransaction(msg);
  }

  // Legacy-compat APIs for simpler DX
  async initializeTxn(payer: PublicKey) {
    const ix = await this.buildInitializeIx(payer);
    return new Transaction().add(ix);
  }

  async createSmartWalletTx(params: {
    payer: PublicKey;
    passkeyPubkey: number[];
    credentialIdBase64: string;
    ruleInstruction?: TransactionInstruction | null;
    isPayForUser?: boolean;
    smartWalletId?: BN;
  }) {
    let smartWalletId: BN = this.generateWalletId();
    if (params.smartWalletId) {
      smartWalletId = params.smartWalletId;
    }
    const smartWallet = this.smartWalletPda(smartWalletId);
    const smartWalletAuthenticator = this.smartWalletAuthenticatorPda(
      smartWallet,
      params.passkeyPubkey
    );

    let ruleInstruction = await this.defaultRuleProgram.buildInitRuleIx(
      params.payer,
      smartWallet,
      smartWalletAuthenticator
    );

    if (params.ruleInstruction) {
      ruleInstruction = params.ruleInstruction;
    }

    const args = {
      passkeyPubkey: params.passkeyPubkey,
      credentialId: Buffer.from(params.credentialIdBase64, 'base64'),
      ruleData: ruleInstruction.data,
      walletId: smartWalletId,
      isPayForUser: params.isPayForUser === true,
    };

    const ix = await this.buildCreateSmartWalletIx(
      params.payer,
      smartWallet,
      smartWalletAuthenticator,
      ruleInstruction,
      args
    );
    const tx = new Transaction().add(ix);
    tx.feePayer = params.payer;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
    return {
      transaction: tx,
      smartWalletId: smartWalletId,
      smartWallet,
    };
  }

  async buildMessage(params: {
    action: types.MessageArgs;
    payer: PublicKey;
    smartWallet: PublicKey;
    passkeyPubkey: number[];
  }): Promise<Buffer<ArrayBufferLike>> {
    let message: Buffer<ArrayBufferLike>;

    const { action, payer, smartWallet, passkeyPubkey } = params;

    switch (action.type) {
      case types.SmartWalletAction.ExecuteTx: {
        const { ruleInstruction: ruleIns, cpiInstruction } =
          action.args as types.ArgsByAction[types.SmartWalletAction.ExecuteTx];

        let ruleInstruction = await this.defaultRuleProgram.buildCheckRuleIx(
          this.smartWalletAuthenticatorPda(smartWallet, passkeyPubkey)
        );

        if (ruleIns) {
          ruleInstruction = ruleIns;
        }

        const smartWalletConfigData = await this.getSmartWalletConfigData(smartWallet);

        message = buildExecuteMessage(
          payer,
          smartWalletConfigData.lastNonce,
          new BN(Math.floor(Date.now() / 1000)),
          ruleInstruction,
          cpiInstruction
        );
        break;
      }
      case types.SmartWalletAction.CallRule: {
        const { ruleInstruction } =
          action.args as types.ArgsByAction[types.SmartWalletAction.CallRule];

        const smartWalletConfigData = await this.getSmartWalletConfigData(smartWallet);

        message = buildCallRuleMessage(
          payer,
          smartWalletConfigData.lastNonce,
          new BN(Math.floor(Date.now() / 1000)),
          ruleInstruction
        );
        break;
      }
      case types.SmartWalletAction.ChangeRule: {
        const { initRuleIns, destroyRuleIns } =
          action.args as types.ArgsByAction[types.SmartWalletAction.ChangeRule];

        const smartWalletConfigData = await this.getSmartWalletConfigData(smartWallet);

        message = buildChangeRuleMessage(
          payer,
          smartWalletConfigData.lastNonce,
          new BN(Math.floor(Date.now() / 1000)),
          destroyRuleIns,
          initRuleIns
        );
        break;
      }

      default:
        throw new Error(`Unsupported SmartWalletAction: ${action.type}`);
    }

    return message;
  }
}
