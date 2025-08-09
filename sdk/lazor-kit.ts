import * as anchor from '@coral-xyz/anchor';
import IDL from '../target/idl/lazorkit.json';
import * as bs58 from 'bs58';
import { Lazorkit } from '../target/types/lazorkit';
import * as constants from './constants';
import {
  createSecp256r1Instruction,
  hashSeeds,
  instructionToAccountMetas,
} from './utils';
import * as types from './types';
import { sha256 } from 'js-sha256';
import { DefaultRuleProgram } from './default-rule-program';
import { Buffer } from 'buffer';
import { sha256 as jsSha256 } from 'js-sha256';

// Polyfill for structuredClone (e.g. React Native/Expo)
if (typeof globalThis.structuredClone !== 'function') {
  // eslint-disable-next-line @typescript-eslint/ban-ts-comment
  // @ts-ignore – minimal polyfill for non-circular data
  globalThis.structuredClone = (obj: unknown) =>
    JSON.parse(JSON.stringify(obj));
}

export class LazorKitProgram {
  readonly connection: anchor.web3.Connection;
  readonly program: anchor.Program<Lazorkit>;
  readonly programId: anchor.web3.PublicKey;

  // Caches for PDAs
  private _config?: anchor.web3.PublicKey;
  private _whitelistRulePrograms?: anchor.web3.PublicKey;
  private _lookupTableAccount?: anchor.web3.AddressLookupTableAccount;

  readonly defaultRuleProgram: DefaultRuleProgram;
  private _executeMsgCoder?: anchor.BorshCoder;

  constructor(connection: anchor.web3.Connection) {
    this.connection = connection;
    this.program = new anchor.Program(IDL as anchor.Idl, {
      connection,
    }) as unknown as anchor.Program<Lazorkit>;
    this.programId = this.program.programId;
    this.defaultRuleProgram = new DefaultRuleProgram(connection);
  }

  // PDA getters
  get config(): anchor.web3.PublicKey {
    if (!this._config) {
      this._config = anchor.web3.PublicKey.findProgramAddressSync(
        [constants.CONFIG_SEED],
        this.programId
      )[0];
    }
    return this._config;
  }

  get whitelistRulePrograms(): anchor.web3.PublicKey {
    if (!this._whitelistRulePrograms) {
      this._whitelistRulePrograms =
        anchor.web3.PublicKey.findProgramAddressSync(
          [constants.WHITELIST_RULE_PROGRAMS_SEED],
          this.programId
        )[0];
    }
    return this._whitelistRulePrograms;
  }

  /**
   * Get or fetch the address lookup table account
   */
  async getLookupTableAccount(): Promise<anchor.web3.AddressLookupTableAccount | null> {
    if (!this._lookupTableAccount) {
      try {
        const response = await this.connection.getAddressLookupTable(
          constants.ADDRESS_LOOKUP_TABLE
        );
        this._lookupTableAccount = response.value;
      } catch (error) {
        console.warn('Failed to fetch lookup table account:', error);
        return null;
      }
    }
    return this._lookupTableAccount;
  }

  /**
   * Generate a random wallet ID
   * Uses timestamp + random number to minimize collision probability
   */
  generateWalletId(): bigint {
    // Use timestamp in milliseconds (lower 48 bits)
    const timestamp = BigInt(Date.now()) & BigInt('0xFFFFFFFFFFFF');

    // Generate random 16 bits
    const randomPart = BigInt(Math.floor(Math.random() * 0xffff));

    // Combine: timestamp (48 bits) + random (16 bits) = 64 bits
    const walletId = (timestamp << BigInt(16)) | randomPart;

    // Ensure it's not zero (reserved)
    return walletId === BigInt(0) ? BigInt(1) : walletId;
  }

  /**
   * Check if a wallet ID already exists on-chain
   */
  private async isWalletIdTaken(walletId: bigint): Promise<boolean> {
    try {
      const smartWalletPda = this.smartWallet(walletId);
      const accountInfo = await this.connection.getAccountInfo(smartWalletPda);
      return accountInfo !== null;
    } catch (error) {
      // If there's an error checking, assume it's not taken
      return false;
    }
  }

  /**
   * Generate a unique wallet ID by checking for collisions
   * Retries up to maxAttempts times if collisions are found
   */
  async generateUniqueWalletId(maxAttempts: number = 10): Promise<bigint> {
    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      const walletId = this.generateWalletId();

      // Check if this ID is already taken
      const isTaken = await this.isWalletIdTaken(walletId);

      if (!isTaken) {
        return walletId;
      }

      // If taken, log and retry
      console.warn(
        `Wallet ID ${walletId} already exists, retrying... (attempt ${
          attempt + 1
        }/${maxAttempts})`
      );

      // Add small delay to avoid rapid retries
      if (attempt < maxAttempts - 1) {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    }

    throw new Error(
      `Failed to generate unique wallet ID after ${maxAttempts} attempts`
    );
  }

  /**
   * Find smart wallet PDA with given ID
   */
  smartWallet(walletId: bigint): anchor.web3.PublicKey {
    const idBytes = new Uint8Array(8);
    const view = new DataView(idBytes.buffer);
    view.setBigUint64(0, walletId, true); // little-endian

    return anchor.web3.PublicKey.findProgramAddressSync(
      [constants.SMART_WALLET_SEED, idBytes],
      this.programId
    )[0];
  }

  smartWalletConfig(smartWallet: anchor.web3.PublicKey) {
    return anchor.web3.PublicKey.findProgramAddressSync(
      [constants.SMART_WALLET_CONFIG_SEED, smartWallet.toBuffer()],
      this.programId
    )[0];
  }

  smartWalletAuthenticator(
    passkeyPubkey: number[],
    smartWallet: anchor.web3.PublicKey
  ) {
    const hashedPasskey = hashSeeds(passkeyPubkey, smartWallet);
    return anchor.web3.PublicKey.findProgramAddressSync(
      [
        constants.SMART_WALLET_AUTHENTICATOR_SEED,
        smartWallet.toBuffer(),
        hashedPasskey,
      ],
      this.programId
    );
  }

  // async methods

  async getConfigData(): Promise<types.Config> {
    return await this.program.account.config.fetch(this.config);
  }

  async getSmartWalletConfigData(smartWallet: anchor.web3.PublicKey) {
    const config = this.smartWalletConfig(smartWallet);
    return await this.program.account.smartWalletConfig.fetch(config);
  }

  async getSmartWalletAuthenticatorData(
    smartWalletAuthenticator: anchor.web3.PublicKey
  ) {
    return await this.program.account.smartWalletAuthenticator.fetch(
      smartWalletAuthenticator
    );
  }

  // Helper method to create versioned transactions
  private async createVersionedTransaction(
    instructions: anchor.web3.TransactionInstruction[],
    payer: anchor.web3.PublicKey
  ): Promise<anchor.web3.VersionedTransaction> {
    const lookupTableAccount = await this.getLookupTableAccount();
    const { blockhash } = await this.connection.getLatestBlockhash();

    // Create v0 compatible transaction message
    const messageV0 = new anchor.web3.TransactionMessage({
      payerKey: payer,
      recentBlockhash: blockhash,
      instructions,
    }).compileToV0Message(lookupTableAccount ? [lookupTableAccount] : []);

    // Create v0 transaction from the v0 message
    return new anchor.web3.VersionedTransaction(messageV0);
  }

  // txn methods

  async initializeTxn(
    payer: anchor.web3.PublicKey
  ): Promise<anchor.web3.Transaction> {
    const ix = await this.program.methods
      .initialize()
      .accountsPartial({
        signer: payer,
        config: this.config,
        whitelistRulePrograms: this.whitelistRulePrograms,
        defaultRuleProgram: this.defaultRuleProgram.programId,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .instruction();
    return new anchor.web3.Transaction().add(ix);
  }

  async updateConfigTxn(
    authority: anchor.web3.PublicKey,
    param: types.UpdateConfigType,
    value: number,
    remainingAccounts: anchor.web3.AccountMeta[] = []
  ): Promise<anchor.web3.Transaction> {
    const ix = await this.program.methods
      .updateConfig(param, new anchor.BN(value))
      .accountsPartial({
        authority,
        config: this._config ?? this.config,
      })
      .remainingAccounts(remainingAccounts)
      .instruction();
    return new anchor.web3.Transaction().add(ix);
  }

  /**
   * Create smart wallet with automatic collision detection
   */
  async createSmartWalletTxn(
    passkeyPubkey: number[],
    payer: anchor.web3.PublicKey,
    credentialId: string = '',
    ruleIns: anchor.web3.TransactionInstruction | null = null,
    walletId?: bigint,
    isPayForUser: boolean = false
  ): Promise<{
    transaction: anchor.web3.Transaction;
    walletId: bigint;
    smartWallet: anchor.web3.PublicKey;
  }> {
    // Generate unique ID if not provided
    const id = walletId ?? (await this.generateUniqueWalletId());

    const smartWallet = this.smartWallet(id);
    const [smartWalletAuthenticator] = this.smartWalletAuthenticator(
      passkeyPubkey,
      smartWallet
    );

    // If caller does not provide a rule instruction, default to initRule of DefaultRuleProgram
    const ruleInstruction =
      ruleIns ||
      (await this.defaultRuleProgram.initRuleIns(
        payer,
        smartWallet,
        smartWalletAuthenticator
      ));

    const remainingAccounts = instructionToAccountMetas(ruleInstruction, payer);

    const createSmartWalletIx = await this.program.methods
      .createSmartWallet(
        passkeyPubkey,
        Buffer.from(credentialId, 'base64'),
        ruleInstruction.data,
        new anchor.BN(id.toString()),
        isPayForUser
      )
      .accountsPartial({
        signer: payer,
        whitelistRulePrograms: this.whitelistRulePrograms,
        smartWallet,
        smartWalletConfig: this.smartWalletConfig(smartWallet),
        smartWalletAuthenticator,
        config: this.config,
        systemProgram: anchor.web3.SystemProgram.programId,
        defaultRuleProgram: this.defaultRuleProgram.programId,
      })
      .remainingAccounts(remainingAccounts)
      .instruction();

    const tx = new anchor.web3.Transaction().add(createSmartWalletIx);
    tx.feePayer = payer;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;

    return {
      transaction: tx,
      walletId: id,
      smartWallet,
    };
  }

  async executeInstructionTxn(
    passkeyPubkey: number[],
    clientDataJsonRaw: Buffer,
    authenticatorDataRaw: Buffer,
    signature: Buffer,
    payer: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    cpiIns: anchor.web3.TransactionInstruction,
    ruleIns: anchor.web3.TransactionInstruction | null = null,
    verifyInstructionIndex: number = 0
  ): Promise<anchor.web3.VersionedTransaction> {
    const [smartWalletAuthenticator] = this.smartWalletAuthenticator(
      passkeyPubkey,
      smartWallet
    );
    const smartWalletConfig = this.smartWalletConfig(smartWallet);
    const smartWalletConfigData = await this.getSmartWalletConfigData(
      smartWallet
    );

    const remainingAccounts: anchor.web3.AccountMeta[] = [];

    let ruleInstruction: anchor.web3.TransactionInstruction | null = null;

    if (!ruleIns) {
      ruleInstruction = await this.defaultRuleProgram.checkRuleIns(
        smartWalletAuthenticator
      );
    } else {
      ruleInstruction = ruleIns;
    }

    if (ruleInstruction) {
      remainingAccounts.push(
        ...instructionToAccountMetas(ruleInstruction, payer)
      );
    }

    remainingAccounts.push(...instructionToAccountMetas(cpiIns, payer));

    const message = Buffer.concat([
      authenticatorDataRaw,
      Buffer.from(sha256.arrayBuffer(clientDataJsonRaw)),
    ]);

    const verifySignatureIx = createSecp256r1Instruction(
      message,
      Buffer.from(passkeyPubkey),
      signature
    );

    const executeInstructionIx = await this.program.methods
      .executeTxnDirect({
        passkeyPubkey,
        signature,
        clientDataJsonRaw,
        authenticatorDataRaw,
        verifyInstructionIndex,
        splitIndex: ruleInstruction.keys.length,
        ruleData: ruleInstruction.data,
        cpiData: cpiIns.data,
      })
      .accountsPartial({
        payer,
        smartWallet,
        smartWalletConfig,
        config: this.config,
        smartWalletAuthenticator,
        whitelistRulePrograms: this.whitelistRulePrograms,
        authenticatorProgram: smartWalletConfigData.ruleProgram,
        ixSysvar: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
        cpiProgram: cpiIns.programId,
      })
      .remainingAccounts(remainingAccounts)
      .instruction();

    return this.createVersionedTransaction(
      [verifySignatureIx, executeInstructionIx],
      payer
    );
  }

  async callRuleDirectTxn(
    passkeyPubkey: number[],
    clientDataJsonRaw: Buffer,
    authenticatorDataRaw: Buffer,
    signature: Buffer,
    payer: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    ruleProgram: anchor.web3.PublicKey,
    ruleIns: anchor.web3.TransactionInstruction,
    verifyInstructionIndex: number = 0,
    newPasskey?: number[] | Buffer,
    newAuthenticator?: anchor.web3.PublicKey
  ): Promise<anchor.web3.VersionedTransaction> {
    const [smartWalletAuthenticator] = this.smartWalletAuthenticator(
      passkeyPubkey,
      smartWallet
    );
    const smartWalletConfig = this.smartWalletConfig(smartWallet);

    // Prepare remaining accounts: optional new authenticator first, then rule accounts
    const remainingAccounts: anchor.web3.AccountMeta[] = [];
    if (newAuthenticator) {
      remainingAccounts.push({
        pubkey: newAuthenticator,
        isWritable: true,
        isSigner: false,
      });
    }
    remainingAccounts.push(...instructionToAccountMetas(ruleIns, payer));

    const message = Buffer.concat([
      authenticatorDataRaw,
      Buffer.from(sha256.arrayBuffer(clientDataJsonRaw)),
    ]);

    const verifySignatureIx = createSecp256r1Instruction(
      message,
      Buffer.from(passkeyPubkey),
      signature
    );

    const ix = await (this.program.methods as any)
      .callRuleDirect({
        passkeyPubkey,
        signature,
        clientDataJsonRaw,
        authenticatorDataRaw,
        verifyInstructionIndex,
        ruleProgram,
        ruleData: ruleIns.data,
        createNewAuthenticator: newPasskey
          ? (Array.from(new Uint8Array(newPasskey as any)) as any)
          : null,
      } as any)
      .accountsPartial({
        payer,
        config: this.config,
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        ruleProgram,
        whitelistRulePrograms: this.whitelistRulePrograms,
        ixSysvar: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts(remainingAccounts)
      .instruction();

    return this.createVersionedTransaction(
      [verifySignatureIx, ix],
      payer
    );
  }

  async changeRuleDirectTxn(
    passkeyPubkey: number[],
    clientDataJsonRaw: Buffer,
    authenticatorDataRaw: Buffer,
    signature: Buffer,
    payer: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    oldRuleProgram: anchor.web3.PublicKey,
    destroyRuleIns: anchor.web3.TransactionInstruction,
    newRuleProgram: anchor.web3.PublicKey,
    initRuleIns: anchor.web3.TransactionInstruction,
    verifyInstructionIndex: number = 0
  ): Promise<anchor.web3.VersionedTransaction> {
    const [smartWalletAuthenticator] = this.smartWalletAuthenticator(
      passkeyPubkey,
      smartWallet
    );
    const smartWalletConfig = this.smartWalletConfig(smartWallet);

    // Build remaining accounts: destroy accounts then init accounts
    const destroyMetas = instructionToAccountMetas(destroyRuleIns, payer);
    const initMetas = instructionToAccountMetas(initRuleIns, payer);
    const remainingAccounts = [...destroyMetas, ...initMetas];
    const splitIndex = destroyMetas.length;

    const message = Buffer.concat([
      authenticatorDataRaw,
      Buffer.from(sha256.arrayBuffer(clientDataJsonRaw)),
    ]);
    const verifySignatureIx = createSecp256r1Instruction(
      message,
      Buffer.from(passkeyPubkey),
      signature
    );

    const ix = await (this.program.methods as any)
      .changeRuleDirect({
        passkeyPubkey,
        signature,
        clientDataJsonRaw,
        authenticatorDataRaw,
        verifyInstructionIndex,
        splitIndex,
        oldRuleProgram,
        destroyRuleData: destroyRuleIns.data,
        newRuleProgram,
        initRuleData: initRuleIns.data,
        createNewAuthenticator: null,
      } as any)
      .accountsPartial({
        payer,
        config: this.config,
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        oldRuleProgram,
        newRuleProgram,
        whitelistRulePrograms: this.whitelistRulePrograms,
        ixSysvar: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts(remainingAccounts)
      .instruction();

    return this.createVersionedTransaction(
      [verifySignatureIx, ix],
      payer
    );
  }

  async getSmartWalletByPasskey(passkeyPubkey: number[]): Promise<{
    smartWallet: anchor.web3.PublicKey | null;
    smartWalletAuthenticator: anchor.web3.PublicKey | null;
  }> {
    const discriminator = (IDL as any).accounts.find(
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

    const smartWalletAuthenticatorData =
      await this.getSmartWalletAuthenticatorData(accounts[0].pubkey);

    return {
      smartWalletAuthenticator: accounts[0].pubkey,
      smartWallet: smartWalletAuthenticatorData.smartWallet,
    };
  }

  async buildExecuteMessage(
    smartWallet: anchor.web3.PublicKey,
    payer: anchor.web3.PublicKey,
    ruleIns: anchor.web3.TransactionInstruction | null,
    cpiIns: anchor.web3.TransactionInstruction
  ): Promise<Buffer> {
    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const nonce = BigInt(cfg.lastNonce.toString());
    const now = Math.floor(Date.now() / 1000);

    // Rule instruction and metas
    let ruleInstruction = ruleIns;
    if (!ruleInstruction) {
      ruleInstruction = await this.defaultRuleProgram.checkRuleIns(smartWallet);
    }
    const ruleMetas = instructionToAccountMetas(ruleInstruction, payer);
    const ruleAccountsHash = this.computeAccountsHash(
      ruleInstruction.programId,
      ruleMetas
    );
    const ruleDataHash = new Uint8Array(
      jsSha256.arrayBuffer(ruleInstruction.data)
    );

    // CPI hashes
    const cpiMetas = instructionToAccountMetas(cpiIns, payer);
    const cpiAccountsHash = this.computeAccountsHash(
      cpiIns.programId,
      cpiMetas
    );
    const cpiDataHash = new Uint8Array(jsSha256.arrayBuffer(cpiIns.data));

    const buf = Buffer.alloc(8 + 8 + 32 * 4);
    buf.writeBigUInt64LE(nonce, 0);
    buf.writeBigInt64LE(BigInt(now), 8);
    Buffer.from(ruleDataHash).copy(buf, 16);
    Buffer.from(ruleAccountsHash).copy(buf, 48);
    Buffer.from(cpiDataHash).copy(buf, 80);
    Buffer.from(cpiAccountsHash).copy(buf, 112);
    return buf;
  }

  async buildExecuteMessageWithCoder(
    smartWallet: anchor.web3.PublicKey,
    payer: anchor.web3.PublicKey,
    ruleIns: anchor.web3.TransactionInstruction | null,
    cpiIns: anchor.web3.TransactionInstruction
  ): Promise<Buffer> {
    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const nonceBn = new anchor.BN(cfg.lastNonce.toString());
    const nowBn = new anchor.BN(Math.floor(Date.now() / 1000));

    // Resolve rule instruction and compute hashes
    let ruleInstruction = ruleIns;
    if (!ruleInstruction) {
      ruleInstruction = await this.defaultRuleProgram.checkRuleIns(smartWallet);
    }
    const ruleMetas = instructionToAccountMetas(ruleInstruction, payer);
    const ruleAccountsHash = this.computeAccountsHash(
      ruleInstruction.programId,
      ruleMetas
    );
    const ruleDataHash = new Uint8Array(
      jsSha256.arrayBuffer(ruleInstruction.data)
    );

    // CPI hashes
    const cpiMetas = instructionToAccountMetas(cpiIns, payer);
    const cpiAccountsHash = this.computeAccountsHash(
      cpiIns.programId,
      cpiMetas
    );
    const cpiDataHash = new Uint8Array(jsSha256.arrayBuffer(cpiIns.data));

    // Encode via BorshCoder using a minimal IDL type definition
    const coder = this.getExecuteMessageCoder();
    const encoded = coder.types.encode('ExecuteMessage', {
      nonce: nonceBn,
      currentTimestamp: nowBn,
      ruleDataHash: Array.from(ruleDataHash),
      ruleAccountsHash: Array.from(
        cpiAccountsHash.length === 32 ? ruleAccountsHash : ruleAccountsHash
      ),
      cpiDataHash: Array.from(cpiDataHash),
      cpiAccountsHash: Array.from(cpiAccountsHash),
    });
    return Buffer.from(encoded);
  }

  private getExecuteMessageCoder(): anchor.BorshCoder {
    if ((this as any)._executeMsgCoder) return (this as any)._executeMsgCoder;
    const idl: any = {
      version: '0.1.0',
      name: 'lazorkit_msgs',
      instructions: [],
      accounts: [],
      types: [
        {
          name: 'ExecuteMessage',
          type: {
            kind: 'struct',
            fields: [
              { name: 'nonce', type: 'u64' },
              { name: 'currentTimestamp', type: 'i64' },
              { name: 'ruleDataHash', type: { array: ['u8', 32] } },
              { name: 'ruleAccountsHash', type: { array: ['u8', 32] } },
              { name: 'cpiDataHash', type: { array: ['u8', 32] } },
              { name: 'cpiAccountsHash', type: { array: ['u8', 32] } },
            ],
          },
        },
        {
          name: 'CallRuleMessage',
          type: {
            kind: 'struct',
            fields: [
              { name: 'nonce', type: 'u64' },
              { name: 'currentTimestamp', type: 'i64' },
              { name: 'ruleDataHash', type: { array: ['u8', 32] } },
              { name: 'ruleAccountsHash', type: { array: ['u8', 32] } },
              { name: 'newPasskey', type: { option: { array: ['u8', 33] } } },
            ],
          },
        },
        {
          name: 'ChangeRuleMessage',
          type: {
            kind: 'struct',
            fields: [
              { name: 'nonce', type: 'u64' },
              { name: 'currentTimestamp', type: 'i64' },
              { name: 'oldRuleDataHash', type: { array: ['u8', 32] } },
              { name: 'oldRuleAccountsHash', type: { array: ['u8', 32] } },
              { name: 'newRuleDataHash', type: { array: ['u8', 32] } },
              { name: 'newRuleAccountsHash', type: { array: ['u8', 32] } },
            ],
          },
        },
      ],
    };
    (this as any)._executeMsgCoder = new anchor.BorshCoder(idl);
    return (this as any)._executeMsgCoder;
  }

  async buildCallRuleMessageWithCoder(
    smartWallet: anchor.web3.PublicKey,
    payer: anchor.web3.PublicKey,
    ruleProgram: anchor.web3.PublicKey,
    ruleIns: anchor.web3.TransactionInstruction,
    newPasskey?: Uint8Array | number[] | Buffer
  ): Promise<Buffer> {
    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const nonceBn = new anchor.BN(cfg.lastNonce.toString());
    const nowBn = new anchor.BN(Math.floor(Date.now() / 1000));
    const ruleMetas = instructionToAccountMetas(ruleIns, payer);
    const ruleAccountsHash = this.computeAccountsHash(ruleProgram, ruleMetas);
    const ruleDataHash = new Uint8Array(jsSha256.arrayBuffer(ruleIns.data));
    const coder = this.getExecuteMessageCoder();
    const encoded = coder.types.encode('CallRuleMessage', {
      nonce: nonceBn,
      currentTimestamp: nowBn,
      ruleDataHash: Array.from(ruleDataHash),
      ruleAccountsHash: Array.from(ruleAccountsHash),
      newPasskey:
        newPasskey && (newPasskey as any).length
          ? Array.from(new Uint8Array(newPasskey as any))
          : null,
    });
    return Buffer.from(encoded);
  }

  async buildChangeRuleMessageWithCoder(
    smartWallet: anchor.web3.PublicKey,
    payer: anchor.web3.PublicKey,
    oldRuleProgram: anchor.web3.PublicKey,
    destroyRuleIns: anchor.web3.TransactionInstruction,
    newRuleProgram: anchor.web3.PublicKey,
    initRuleIns: anchor.web3.TransactionInstruction
  ): Promise<Buffer> {
    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const nonceBn = new anchor.BN(cfg.lastNonce.toString());
    const nowBn = new anchor.BN(Math.floor(Date.now() / 1000));
    const oldMetas = instructionToAccountMetas(destroyRuleIns, payer);
    const oldAccountsHash = this.computeAccountsHash(oldRuleProgram, oldMetas);
    const oldDataHash = new Uint8Array(
      jsSha256.arrayBuffer(destroyRuleIns.data)
    );
    const newMetas = instructionToAccountMetas(initRuleIns, payer);
    const newAccountsHash = this.computeAccountsHash(newRuleProgram, newMetas);
    const newDataHash = new Uint8Array(jsSha256.arrayBuffer(initRuleIns.data));
    const coder = this.getExecuteMessageCoder();
    const encoded = coder.types.encode('ChangeRuleMessage', {
      nonce: nonceBn,
      currentTimestamp: nowBn,
      oldRuleDataHash: Array.from(oldDataHash),
      oldRuleAccountsHash: Array.from(oldAccountsHash),
      newRuleDataHash: Array.from(newDataHash),
      newRuleAccountsHash: Array.from(newAccountsHash),
    });
    return Buffer.from(encoded);
  }

  private computeAccountsHash(
    cpiProgram: anchor.web3.PublicKey,
    accountMetas: anchor.web3.AccountMeta[]
  ): Uint8Array {
    const h = sha256.create();
    h.update(cpiProgram.toBytes());
    for (const m of accountMetas) {
      h.update(m.pubkey.toBytes());
      h.update(Uint8Array.from([m.isWritable ? 1 : 0, m.isSigner ? 1 : 0]));
    }
    return new Uint8Array(h.arrayBuffer());
  }

  async commitCpiTxn(
    passkeyPubkey: number[],
    clientDataJsonRaw: Buffer,
    authenticatorDataRaw: Buffer,
    signature: Buffer,
    payer: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    cpiProgram: anchor.web3.PublicKey,
    ruleIns: anchor.web3.TransactionInstruction | undefined,
    expiresAt: number,
    verifyInstructionIndex: number = 0
  ): Promise<anchor.web3.Transaction> {
    const [smartWalletAuthenticator] = this.smartWalletAuthenticator(
      passkeyPubkey,
      smartWallet
    );
    const smartWalletConfig = this.smartWalletConfig(smartWallet);
    const smartWalletConfigData = await this.getSmartWalletConfigData(
      smartWallet
    );

    let ruleInstruction: anchor.web3.TransactionInstruction | null = null;

    if (!ruleIns) {
      ruleInstruction = await this.defaultRuleProgram.checkRuleIns(
        smartWalletAuthenticator
      );
    } else {
      ruleInstruction = ruleIns;
    }

    // In commit mode, only rule accounts are passed for hashing and CPI verification on-chain
    const ruleMetas = instructionToAccountMetas(ruleInstruction, payer);
    const remainingAccounts = [...ruleMetas];

    const message = Buffer.concat([
      authenticatorDataRaw,
      Buffer.from(sha256.arrayBuffer(clientDataJsonRaw)),
    ]);

    const verifySignatureIx = createSecp256r1Instruction(
      message,
      Buffer.from(passkeyPubkey),
      signature
    );

    const ix = await this.program.methods
      .commitCpi({
        passkeyPubkey,
        signature,
        clientDataJsonRaw,
        authenticatorDataRaw,
        verifyInstructionIndex,
        ruleData: ruleInstruction!.data,
        cpiProgram,
        expiresAt: new anchor.BN(expiresAt),
      } as any)
      .accountsPartial({
        payer,
        config: this.config,
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        whitelistRulePrograms: this.whitelistRulePrograms,
        authenticatorProgram: smartWalletConfigData.ruleProgram,
        ixSysvar: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts(remainingAccounts)
      .instruction();

    const tx = new anchor.web3.Transaction().add(verifySignatureIx).add(ix);
    tx.feePayer = payer;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
    return tx;
  }

  async executeCommittedTxn(
    payer: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    cpiIns: anchor.web3.TransactionInstruction
  ): Promise<anchor.web3.VersionedTransaction> {
    const metas = instructionToAccountMetas(cpiIns, payer);
    const smartWalletConfigData = await this.getSmartWalletConfigData(
      smartWallet
    );
    const commitPda = anchor.web3.PublicKey.findProgramAddressSync(
      [
        constants.CPI_COMMIT_SEED,
        smartWallet.toBuffer(),
        smartWalletConfigData.lastNonce.toArrayLike(Buffer, 'le', 8),
      ],
      this.programId
    )[0];

    const ix = await this.program.methods
      .executeCommitted({ cpiData: cpiIns.data } as any)
      .accountsPartial({
        payer,
        config: this.config,
        smartWallet,
        smartWalletConfig: this.smartWalletConfig(smartWallet),
        cpiProgram: cpiIns.programId,
        cpiCommit: commitPda,
        commitRefund: payer,
      })
      .remainingAccounts(metas)
      .instruction();

    return this.createVersionedTransaction([ix], payer);
  }
}
