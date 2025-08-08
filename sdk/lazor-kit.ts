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
import { PublicKey } from '@solana/web3.js';

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
  async isWalletIdTaken(walletId: bigint): Promise<boolean> {
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

  async generateUniqueWalletIdWithRetry(
    maxAttempts: number = 10
  ): Promise<PublicKey> {
    const walletId = await this.generateUniqueWalletId(maxAttempts);
    return this.smartWallet(walletId);
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

  /**
   * Create smart wallet with retry logic for collision handling
   */
  async createSmartWalletWithRetry(
    passkeyPubkey: number[],
    payer: anchor.web3.PublicKey,
    credentialId: string = '',
    ruleIns: anchor.web3.TransactionInstruction | null = null,
    maxRetries: number = 3
  ): Promise<{
    transaction: anchor.web3.Transaction;
    walletId: bigint;
    smartWallet: anchor.web3.PublicKey;
  }> {
    let lastError: Error | null = null;

    for (let attempt = 0; attempt < maxRetries; attempt++) {
      try {
        return await this.createSmartWalletTxn(
          passkeyPubkey,
          payer,
          credentialId,
          ruleIns
        );
      } catch (error) {
        lastError = error as Error;

        // Check if this is a collision error (account already exists)
        if (
          error instanceof Error &&
          error.message.includes('already in use')
        ) {
          console.warn(
            `Wallet creation failed due to collision, retrying... (attempt ${
              attempt + 1
            }/${maxRetries})`
          );
          continue;
        }

        // If it's not a collision error, don't retry
        throw error;
      }
    }

    throw new Error(
      `Failed to create smart wallet after ${maxRetries} attempts. Last error: ${lastError?.message}`
    );
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
    action: types.ExecuteActionType = types.ExecuteAction.ExecuteTx,
    newPasskey: number[] | null = null,
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

    if (action == types.ExecuteAction.ExecuteTx) {
      if (!ruleIns) {
        ruleInstruction = await this.defaultRuleProgram.checkRuleIns(
          smartWalletAuthenticator
        );
      } else {
        ruleInstruction = ruleIns;
      }
    } else if (action == types.ExecuteAction.ChangeRuleProgram) {
      if (!ruleIns) {
        throw new Error('Rule instruction is required');
      }
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
      .execute({
        passkeyPubkey,
        signature,
        clientDataJsonRaw,
        authenticatorDataRaw,
        verifyInstructionIndex,
        action,
        createNewAuthenticator: newPasskey,
      })
      .accountsPartial({
        payer,
        config: this.config,
        smartWallet,
        smartWalletConfig: smartWalletConfig,
        smartWalletAuthenticator,
        whitelistRulePrograms: this.whitelistRulePrograms,
        authenticatorProgram: smartWalletConfigData.ruleProgram,
        ixSysvar: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: anchor.web3.SystemProgram.programId,
        cpiProgram: cpiIns
          ? cpiIns.programId
          : anchor.web3.SystemProgram.programId,
        newSmartWalletAuthenticator: newPasskey
          ? new anchor.web3.PublicKey(
              this.smartWalletAuthenticator(newPasskey, smartWallet)[0]
            )
          : anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts(remainingAccounts)
      .instruction();

    return this.createVersionedTransaction(
      [verifySignatureIx, executeInstructionIx],
      payer
    );
  }

  // Old blob helpers removed; using commit/executeCommitted below

  /**
   * Query the chain for the smart-wallet associated with a passkey.
   */
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

  /**
   * Build the serialized Message struct used for signing requests.
   */
  async getMessage(
    smartWalletString: string,
    ruleIns: anchor.web3.TransactionInstruction | null = null,
    cpiInstruction: anchor.web3.TransactionInstruction,
    executeAction: types.ExecuteActionType
  ): Promise<Buffer> {
    const smartWallet = new anchor.web3.PublicKey(smartWalletString);
    const smartWalletData = await this.getSmartWalletConfigData(smartWallet);

    let ruleInstruction: anchor.web3.TransactionInstruction | null = null;

    if (executeAction == types.ExecuteAction.ChangeRuleProgram) {
      if (!ruleIns) {
        throw new Error('Rule instruction is required');
      }
      ruleInstruction = ruleIns;
    } else if (executeAction == types.ExecuteAction.ExecuteTx) {
      if (!ruleIns) {
        ruleInstruction = await this.defaultRuleProgram.checkRuleIns(
          smartWallet
        );
      } else {
        ruleInstruction = ruleIns;
      }
    }

    // Manually serialize the message struct:
    // - nonce (u64): 8 bytes
    // - current_timestamp (i64): 8 bytes (unix seconds)
    // - split_index (u16): 2 bytes
    // - rule_data (Option<Vec<u8>>): 1 byte (Some/None) + 4 bytes length + data bytes (if Some)
    // - cpi_data_hash (Option<[u8;32]>): 1 byte tag (0=None,1=Some + 32 bytes)
    // - cpi_data (Vec<u8>): 4 bytes length + data bytes

    const currentTimestamp = Math.floor(Date.now() / 1000);

    // Calculate buffer size based on whether rule_data is provided
    const ruleDataLength = ruleInstruction ? ruleInstruction.data.length : 0;
    const ruleDataSize = ruleInstruction ? 5 + ruleDataLength : 1; // 1 byte for Option + 4 bytes length + data (if Some)
    // None(cpi_data_hash): only 1 byte tag
    const buffer = Buffer.alloc(
      18 + ruleDataSize + 1 + 4 + cpiInstruction.data.length
    );

    // Write nonce as little-endian u64 (bytes 0-7)
    buffer.writeBigUInt64LE(BigInt(smartWalletData.lastNonce.toString()), 0);

    // Write current_timestamp as little-endian i64 (bytes 8-15)
    buffer.writeBigInt64LE(BigInt(currentTimestamp), 8);

    // Write split_index as little-endian u16 (bytes 16-17)
    const splitIndex = ruleInstruction ? ruleInstruction.keys.length : 0;
    buffer.writeUInt16LE(splitIndex, 16);

    // Write rule_data (Option<Vec<u8>>)
    if (ruleInstruction) {
      // Write Some variant (1 byte)
      buffer.writeUInt8(1, 18);
      // Write rule_data length as little-endian u32 (bytes 19-22)
      buffer.writeUInt32LE(ruleInstruction.data.length, 19);
      // Write rule_data bytes (starting at byte 23)
      ruleInstruction.data.copy(buffer, 23);
    } else {
      // Write None variant (1 byte)
      buffer.writeUInt8(0, 18);
    }

    // Write cpi_data_hash as None (no blob) -> 1 byte 0
    const afterHashOffset = 18 + ruleDataSize;
    buffer.writeUInt8(0, afterHashOffset); // None

    // Write cpi_data length as little-endian u32
    const cpiDataOffset = afterHashOffset + 1;
    buffer.writeUInt32LE(cpiInstruction.data.length, cpiDataOffset);

    // Write cpi_data bytes
    cpiInstruction.data.copy(buffer, cpiDataOffset + 4);

    return buffer;
  }

  /** Build Message buffer with a blob hash instead of inline cpi_data */
  async getMessageWithBlob(): Promise<Buffer> {
    throw new Error('Deprecated: use commitCpiTxn/executeCommittedTxn instead');
  }

  // === New commit/executeCommitted API ===

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

  commitPda(ownerWallet: anchor.web3.PublicKey, dataHash: Uint8Array) {
    return anchor.web3.PublicKey.findProgramAddressSync(
      [
        constants.CPI_COMMIT_SEED,
        ownerWallet.toBuffer(),
        Buffer.from(dataHash),
      ],
      this.programId
    )[0];
  }

  async commitCpiTxn(
    passkeyPubkey: number[],
    clientDataJsonRaw: Buffer,
    authenticatorDataRaw: Buffer,
    signature: Buffer,
    payer: anchor.web3.PublicKey,
    smartWallet: anchor.web3.PublicKey,
    cpiProgram: anchor.web3.PublicKey,
    cpiIns: anchor.web3.TransactionInstruction,
    ruleIns: anchor.web3.TransactionInstruction | undefined,
    expiresAt: number,
    verifyInstructionIndex: number = 0
  ): Promise<anchor.web3.Transaction> {
    const [smartWalletAuthenticator] = this.smartWalletAuthenticator(
      passkeyPubkey,
      smartWallet
    );
    const smartWalletConfig = this.smartWalletConfig(smartWallet);

    let ruleInstruction: anchor.web3.TransactionInstruction | null = null;

    if (!ruleIns) {
      ruleInstruction = await this.defaultRuleProgram.checkRuleIns(
        smartWalletAuthenticator
      );
    } else {
      ruleInstruction = ruleIns;
    }

    const ruleMetas = instructionToAccountMetas(ruleInstruction, payer);
    const cpiMetas = instructionToAccountMetas(cpiIns, payer);
    const remainingAccounts = [...ruleMetas, ...cpiMetas];
    const accountsHash = this.computeAccountsHash(cpiProgram, cpiMetas);
    const dataHash = new Uint8Array(sha256.arrayBuffer(cpiIns.data));
    const cpiCommit = this.commitPda(smartWallet, dataHash);

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
        splitIndex: ruleMetas.length,
        ruleData: ruleIns ? ruleIns.data : null,
        cpiProgram,
        cpiAccountsHash: Array.from(accountsHash),
        cpiDataHash: Array.from(dataHash),
        expiresAt: new anchor.BN(expiresAt),
      } as any)
      .accountsPartial({
        payer,
        config: this.config,
        smartWallet,
        smartWalletConfig,
        smartWalletAuthenticator,
        whitelistRulePrograms: this.whitelistRulePrograms,
        authenticatorProgram: (
          await this.getSmartWalletConfigData(smartWallet)
        ).ruleProgram,
        cpiCommit,
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
    cpiProgram: anchor.web3.PublicKey,
    cpiIns: anchor.web3.TransactionInstruction
  ): Promise<anchor.web3.VersionedTransaction> {
    const dataHash = new Uint8Array(sha256.arrayBuffer(cpiIns.data));
    const cpiCommit = this.commitPda(smartWallet, dataHash);
    const metas = instructionToAccountMetas(cpiIns, payer);

    const ix = await this.program.methods
      .executeCommitted({ cpiData: cpiIns.data } as any)
      .accountsPartial({
        payer,
        config: this.config,
        smartWallet,
        smartWalletConfig: this.smartWalletConfig(smartWallet),
        cpiProgram,
        cpiCommit,
        commitRefund: payer,
      })
      .remainingAccounts(metas)
      .instruction();

    return this.createVersionedTransaction([ix], payer);
  }
}
