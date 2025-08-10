import * as anchor from '@coral-xyz/anchor';
import {
  Connection,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  VersionedTransaction,
  TransactionMessage,
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
import { decodeAnchorError, SDKError } from '../errors';

export type LazorkitClientOptions = {
  connection: Connection;
  programId?: PublicKey;
};

export class LazorkitClient {
  readonly connection: Connection;
  readonly program: anchor.Program<Lazorkit>;
  readonly programId: PublicKey;

  constructor(opts: LazorkitClientOptions) {
    this.connection = opts.connection;
    const programDefault = new anchor.Program(LazorkitIdl as anchor.Idl, {
      connection: opts.connection,
    }) as unknown as anchor.Program<Lazorkit>;
    this.programId = opts.programId ?? programDefault.programId;
    // Bind program with explicit programId (cast to any to satisfy TS constructor overloads)
    this.program = new (anchor as any).Program(
      LazorkitIdl as anchor.Idl,
      this.programId,
      { connection: opts.connection }
    ) as anchor.Program<Lazorkit>;
  }

  // PDAs
  configPda(): PublicKey {
    return deriveConfigPda(this.programId);
  }
  whitelistRuleProgramsPda(): PublicKey {
    return deriveWhitelistRuleProgramsPda(this.programId);
  }
  smartWalletPda(walletId: bigint): PublicKey {
    return deriveSmartWalletPda(this.programId, walletId);
  }
  smartWalletConfigPda(smartWallet: PublicKey): PublicKey {
    return deriveSmartWalletConfigPda(this.programId, smartWallet);
  }
  smartWalletAuthenticatorPda(
    smartWallet: PublicKey,
    passkey: Uint8Array
  ): PublicKey {
    return deriveSmartWalletAuthenticatorPda(
      this.programId,
      smartWallet,
      passkey
    )[0];
  }
  cpiCommitPda(smartWallet: PublicKey, nonceLe8: Buffer): PublicKey {
    return deriveCpiCommitPda(this.programId, smartWallet, nonceLe8);
  }

  // Convenience helpers
  generateWalletId(): bigint {
    const timestamp = BigInt(Date.now()) & BigInt('0xFFFFFFFFFFFF');
    const randomPart = BigInt(Math.floor(Math.random() * 0xffff));
    const id = (timestamp << BigInt(16)) | randomPart;
    return id === BigInt(0) ? BigInt(1) : id;
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
    try {
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
    } catch (e) {
      throw decodeAnchorError(e);
    }
  }

  async buildCreateSmartWalletIx(params: {
    payer: PublicKey;
    smartWalletId: bigint;
    passkey33: Uint8Array;
    credentialIdBase64: string;
    ruleInstruction: TransactionInstruction;
    isPayForUser?: boolean;
    defaultRuleProgram: PublicKey;
  }): Promise<TransactionInstruction> {
    const {
      payer,
      smartWalletId,
      passkey33,
      credentialIdBase64,
      ruleInstruction,
      isPayForUser = false,
      defaultRuleProgram,
    } = params;
    const smartWallet = this.smartWalletPda(smartWalletId);
    const smartWalletConfig = this.smartWalletConfigPda(smartWallet);
    const smartWalletAuthenticator = this.smartWalletAuthenticatorPda(
      smartWallet,
      passkey33
    );
    return await this.program.methods
      .createSmartWallet(
        Array.from(passkey33),
        Buffer.from(credentialIdBase64, 'base64'),
        ruleInstruction.data,
        new anchor.BN(smartWalletId.toString()),
        isPayForUser
      )
      .accountsPartial({
        signer: payer,
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        config: this.configPda(),
        defaultRuleProgram,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(
        ruleInstruction.keys.map((k) => ({
          pubkey: k.pubkey,
          isWritable: k.isWritable,
          isSigner: k.isSigner,
        }))
      )
      .instruction();
  }

  async buildExecuteTxnDirectIx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    ruleInstruction: TransactionInstruction;
    cpiInstruction: TransactionInstruction;
    verifyInstructionIndex?: number; // default 0
    ruleProgram?: PublicKey; // if omitted, fetched from smartWalletConfig
  }): Promise<TransactionInstruction> {
    const {
      payer,
      smartWallet,
      passkey33,
      signature64,
      clientDataJsonRaw,
      authenticatorDataRaw,
      ruleInstruction,
      cpiInstruction,
      verifyInstructionIndex = 0,
      ruleProgram,
    } = params;

    const smartWalletConfig = this.smartWalletConfigPda(smartWallet);
    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const smartWalletAuthenticator = this.smartWalletAuthenticatorPda(
      smartWallet,
      passkey33
    );

    const remaining = [
      ...ruleInstruction.keys.map((k) => ({
        pubkey: k.pubkey,
        isWritable: k.isWritable,
        isSigner: k.isSigner,
      })),
      ...cpiInstruction.keys.map((k) => ({
        pubkey: k.pubkey,
        isWritable: k.isWritable,
        isSigner: k.isSigner,
      })),
    ];

    const splitIndex = ruleInstruction.keys.length;
    const actualRuleProgram = ruleProgram ?? (cfg.ruleProgram as PublicKey);

    return await (this.program.methods as any)
      .executeTxnDirect({
        passkeyPubkey: Array.from(passkey33),
        signature: Buffer.from(signature64),
        clientDataJsonRaw: Buffer.from(clientDataJsonRaw),
        authenticatorDataRaw: Buffer.from(authenticatorDataRaw),
        verifyInstructionIndex,
        splitIndex,
        ruleData: ruleInstruction.data,
        cpiData: cpiInstruction.data,
      })
      .accountsPartial({
        payer,
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        authenticatorProgram: actualRuleProgram,
        cpiProgram: cpiInstruction.programId,
        config: this.configPda(),
        ixSysvar: (anchor.web3 as any).SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  async buildCallRuleDirectIx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    ruleProgram: PublicKey;
    ruleInstruction: TransactionInstruction;
    verifyInstructionIndex?: number; // default 0
    newPasskey33?: Uint8Array; // optional
    newAuthenticatorPda?: PublicKey; // optional
  }): Promise<TransactionInstruction> {
    const {
      payer,
      smartWallet,
      passkey33,
      signature64,
      clientDataJsonRaw,
      authenticatorDataRaw,
      ruleProgram,
      ruleInstruction,
      verifyInstructionIndex = 0,
      newPasskey33,
      newAuthenticatorPda,
    } = params;

    const smartWalletConfig = this.smartWalletConfigPda(smartWallet);
    const smartWalletAuthenticator = this.smartWalletAuthenticatorPda(
      smartWallet,
      passkey33
    );

    const remaining: {
      pubkey: PublicKey;
      isWritable: boolean;
      isSigner: boolean;
    }[] = [];
    if (newAuthenticatorPda) {
      remaining.push({
        pubkey: newAuthenticatorPda,
        isWritable: true,
        isSigner: false,
      });
    }
    remaining.push(
      ...ruleInstruction.keys.map((k) => ({
        pubkey: k.pubkey,
        isWritable: k.isWritable,
        isSigner: k.isSigner,
      }))
    );

    return await (this.program.methods as any)
      .callRuleDirect({
        passkeyPubkey: Array.from(passkey33),
        signature: Buffer.from(signature64),
        clientDataJsonRaw: Buffer.from(clientDataJsonRaw),
        authenticatorDataRaw: Buffer.from(authenticatorDataRaw),
        verifyInstructionIndex,
        ruleProgram,
        ruleData: ruleInstruction.data,
        createNewAuthenticator: newPasskey33 ? Array.from(newPasskey33) : null,
      })
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        ruleProgram,
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        ixSysvar: (anchor.web3 as any).SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  async buildChangeRuleDirectIx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    oldRuleProgram: PublicKey;
    destroyRuleInstruction: TransactionInstruction;
    newRuleProgram: PublicKey;
    initRuleInstruction: TransactionInstruction;
    verifyInstructionIndex?: number; // default 0
  }): Promise<TransactionInstruction> {
    const {
      payer,
      smartWallet,
      passkey33,
      signature64,
      clientDataJsonRaw,
      authenticatorDataRaw,
      oldRuleProgram,
      destroyRuleInstruction,
      newRuleProgram,
      initRuleInstruction,
      verifyInstructionIndex = 0,
    } = params;

    const smartWalletConfig = this.smartWalletConfigPda(smartWallet);
    const smartWalletAuthenticator = this.smartWalletAuthenticatorPda(
      smartWallet,
      passkey33
    );

    const destroyMetas = destroyRuleInstruction.keys.map((k) => ({
      pubkey: k.pubkey,
      isWritable: k.isWritable,
      isSigner: k.isSigner,
    }));
    const initMetas = initRuleInstruction.keys.map((k) => ({
      pubkey: k.pubkey,
      isWritable: k.isWritable,
      isSigner: k.isSigner,
    }));
    const remaining = [...destroyMetas, ...initMetas];
    const splitIndex = destroyMetas.length;

    return await (this.program.methods as any)
      .changeRuleDirect({
        passkeyPubkey: Array.from(passkey33),
        signature: Buffer.from(signature64),
        clientDataJsonRaw: Buffer.from(clientDataJsonRaw),
        authenticatorDataRaw: Buffer.from(authenticatorDataRaw),
        verifyInstructionIndex,
        splitIndex,
        oldRuleProgram,
        destroyRuleData: destroyRuleInstruction.data,
        newRuleProgram,
        initRuleData: initRuleInstruction.data,
        createNewAuthenticator: null,
      })
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        oldRuleProgram,
        newRuleProgram,
        whitelistRulePrograms: this.whitelistRuleProgramsPda(),
        ixSysvar: (anchor.web3 as any).SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  async buildCommitCpiIx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    ruleInstruction: TransactionInstruction;
    cpiProgram: PublicKey;
    expiresAt: number;
    verifyInstructionIndex?: number;
  }): Promise<TransactionInstruction> {
    const {
      payer,
      smartWallet,
      passkey33,
      signature64,
      clientDataJsonRaw,
      authenticatorDataRaw,
      ruleInstruction,
      cpiProgram,
      expiresAt,
      verifyInstructionIndex = 0,
    } = params;

    const smartWalletConfig = this.smartWalletConfigPda(smartWallet);
    const smartWalletAuthenticator = this.smartWalletAuthenticatorPda(
      smartWallet,
      passkey33
    );

    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const whitelist = this.whitelistRuleProgramsPda();
    const ruleProgram = cfg.ruleProgram as PublicKey;

    const remaining = ruleInstruction.keys.map((k) => ({
      pubkey: k.pubkey,
      isWritable: k.isWritable,
      isSigner: k.isSigner,
    }));

    return await (this.program.methods as any)
      .commitCpi({
        passkeyPubkey: Array.from(passkey33),
        signature: Buffer.from(signature64),
        clientDataJsonRaw: Buffer.from(clientDataJsonRaw),
        authenticatorDataRaw: Buffer.from(authenticatorDataRaw),
        verifyInstructionIndex,
        ruleData: ruleInstruction.data,
        cpiProgram,
        expiresAt: new anchor.BN(expiresAt),
      })
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        whitelistRulePrograms: whitelist,
        authenticatorProgram: ruleProgram,
        ixSysvar: (anchor.web3 as any).SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  async buildExecuteCommittedIx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    cpiInstruction: TransactionInstruction;
  }): Promise<TransactionInstruction> {
    const { payer, smartWallet, cpiInstruction } = params;
    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const nonceLe8 = Buffer.alloc(8);
    nonceLe8.writeBigUInt64LE(BigInt(cfg.lastNonce.toString()));
    const cpiCommit = this.cpiCommitPda(smartWallet, nonceLe8);
    return await (this.program.methods as any)
      .executeCommitted({ cpiData: cpiInstruction.data })
      .accountsPartial({
        payer,
        config: this.configPda(),
        smartWallet,
        smartWalletConfig: this.smartWalletConfigPda(smartWallet),
        cpiProgram: cpiInstruction.programId,
        cpiCommit,
        commitRefund: payer,
      })
      .remainingAccounts(
        cpiInstruction.keys.map((k) => ({
          pubkey: k.pubkey,
          isWritable: k.isWritable,
          isSigner: k.isSigner,
        }))
      )
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
        Buffer.from(
          (anchor as any).utils.sha256.hash(params.clientDataJsonRaw),
          'hex'
        ),
      ]),
      Buffer.from(params.passkey33),
      Buffer.from(params.signature64)
    );
    const execIx = await this.buildExecuteTxnDirectIx({
      payer: params.payer,
      smartWallet: params.smartWallet,
      passkey33: params.passkey33,
      signature64: params.signature64,
      clientDataJsonRaw: params.clientDataJsonRaw,
      authenticatorDataRaw: params.authenticatorDataRaw,
      ruleInstruction: params.ruleInstruction,
      cpiInstruction: params.cpiInstruction,
      verifyInstructionIndex: 0,
      ruleProgram: params.ruleProgram,
    });
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
    newPasskey33?: Uint8Array;
    newAuthenticatorPda?: PublicKey;
  }): Promise<VersionedTransaction> {
    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([
        Buffer.from(params.authenticatorDataRaw),
        Buffer.from(
          (anchor as any).utils.sha256.hash(params.clientDataJsonRaw),
          'hex'
        ),
      ]),
      Buffer.from(params.passkey33),
      Buffer.from(params.signature64)
    );
    const ix = await this.buildCallRuleDirectIx({
      payer: params.payer,
      smartWallet: params.smartWallet,
      passkey33: params.passkey33,
      signature64: params.signature64,
      clientDataJsonRaw: params.clientDataJsonRaw,
      authenticatorDataRaw: params.authenticatorDataRaw,
      ruleProgram: params.ruleProgram,
      ruleInstruction: params.ruleInstruction,
      verifyInstructionIndex: 0,
      newPasskey33: params.newPasskey33,
      newAuthenticatorPda: params.newAuthenticatorPda,
    });
    return this.buildV0Tx(params.payer, [verifyIx, ix]);
  }

  async changeRuleDirectTx(params: {
    payer: PublicKey;
    smartWallet: PublicKey;
    passkey33: Uint8Array;
    signature64: Uint8Array;
    clientDataJsonRaw: Uint8Array;
    authenticatorDataRaw: Uint8Array;
    oldRuleProgram: PublicKey;
    destroyRuleInstruction: TransactionInstruction;
    newRuleProgram: PublicKey;
    initRuleInstruction: TransactionInstruction;
  }): Promise<VersionedTransaction> {
    const verifyIx = buildSecp256r1VerifyIx(
      Buffer.concat([
        Buffer.from(params.authenticatorDataRaw),
        Buffer.from(
          (anchor as any).utils.sha256.hash(params.clientDataJsonRaw),
          'hex'
        ),
      ]),
      Buffer.from(params.passkey33),
      Buffer.from(params.signature64)
    );
    const ix = await this.buildChangeRuleDirectIx({
      payer: params.payer,
      smartWallet: params.smartWallet,
      passkey33: params.passkey33,
      signature64: params.signature64,
      clientDataJsonRaw: params.clientDataJsonRaw,
      authenticatorDataRaw: params.authenticatorDataRaw,
      oldRuleProgram: params.oldRuleProgram,
      destroyRuleInstruction: params.destroyRuleInstruction,
      newRuleProgram: params.newRuleProgram,
      initRuleInstruction: params.initRuleInstruction,
      verifyInstructionIndex: 0,
    });
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
        Buffer.from(
          (anchor as any).utils.sha256.hash(params.clientDataJsonRaw),
          'hex'
        ),
      ]),
      Buffer.from(params.passkey33),
      Buffer.from(params.signature64)
    );
    const ix = await this.buildCommitCpiIx({
      payer: params.payer,
      smartWallet: params.smartWallet,
      passkey33: params.passkey33,
      signature64: params.signature64,
      clientDataJsonRaw: params.clientDataJsonRaw,
      authenticatorDataRaw: params.authenticatorDataRaw,
      ruleInstruction: params.ruleInstruction,
      cpiProgram: params.cpiProgram,
      expiresAt: params.expiresAt,
      verifyInstructionIndex: 0,
    });
    const tx = new (anchor.web3 as any).Transaction().add(verifyIx).add(ix);
    tx.feePayer = params.payer;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
    return tx;
  }

  // Convenience: VersionedTransaction v0
  async buildV0Tx(
    payer: PublicKey,
    ixs: TransactionInstruction[]
  ): Promise<VersionedTransaction> {
    try {
      const { blockhash } = await this.connection.getLatestBlockhash();
      const msg = new TransactionMessage({
        payerKey: payer,
        recentBlockhash: blockhash,
        instructions: ixs,
      }).compileToV0Message();
      return new VersionedTransaction(msg);
    } catch (e) {
      throw new SDKError('Failed to build v0 transaction', e as any);
    }
  }

  // Legacy-compat APIs for simpler DX
  async initializeTxn(payer: PublicKey, defaultRuleProgram: PublicKey) {
    const ix = await this.buildInitializeIx(payer, defaultRuleProgram);
    return new anchor.web3.Transaction().add(ix);
  }

  async createSmartWalletTx(params: {
    payer: PublicKey;
    smartWalletId: bigint;
    passkey33: Uint8Array;
    credentialIdBase64: string;
    ruleInstruction: TransactionInstruction;
    isPayForUser?: boolean;
    defaultRuleProgram: PublicKey;
  }) {
    const ix = await this.buildCreateSmartWalletIx(params);
    const tx = new anchor.web3.Transaction().add(ix);
    tx.feePayer = params.payer;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
    const smartWallet = this.smartWalletPda(params.smartWalletId);
    return {
      transaction: tx,
      smartWalletId: params.smartWalletId,
      smartWallet,
    };
  }

  async simulate(ixs: TransactionInstruction[], payer: PublicKey) {
    try {
      const v0 = await this.buildV0Tx(payer, ixs);
      // Empty signatures for simulate
      return await this.connection.simulateTransaction(v0, {
        sigVerify: false,
      });
    } catch (e) {
      throw decodeAnchorError(e);
    }
  }
}
