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
import * as borsh from 'borsh';
import { Buffer } from 'buffer';

// Polyfill for structuredClone (e.g. React Native/Expo)
if (typeof globalThis.structuredClone !== 'function') {
  // eslint-disable-next-line @typescript-eslint/ban-ts-comment
  // @ts-ignore – minimal polyfill for non-circular data
  globalThis.structuredClone = (obj: unknown) =>
    JSON.parse(JSON.stringify(obj));
}

export class LazorKitProgram {
  /** Network connection used by all RPC / account queries */
  readonly connection: anchor.web3.Connection;

  /** Cached IDL to avoid repeated JSON parsing */
  readonly Idl: anchor.Idl = IDL as Lazorkit;

  /** Lazily-instantiated (and cached) Anchor `Program` wrapper */
  private _program?: anchor.Program<Lazorkit>;

  /** Frequently-used PDA caches (network-independent, so safe to memoise) */
  private _smartWalletSeq?: anchor.web3.PublicKey;
  private _whitelistRulePrograms?: anchor.web3.PublicKey;
  private _config?: anchor.web3.PublicKey;

  readonly defaultRuleProgram: DefaultRuleProgram;

  constructor(connection: anchor.web3.Connection) {
    this.connection = connection;
    this.defaultRuleProgram = new DefaultRuleProgram(connection);
  }

  get program(): anchor.Program<Lazorkit> {
    if (!this._program) {
      this._program = new anchor.Program(this.Idl, {
        connection: this.connection,
      });
    }
    return this._program;
  }

  get programId(): anchor.web3.PublicKey {
    return this.program.programId;
  }

  get smartWalletSeq(): anchor.web3.PublicKey {
    if (!this._smartWalletSeq) {
      this._smartWalletSeq = anchor.web3.PublicKey.findProgramAddressSync(
        [constants.SMART_WALLET_SEQ_SEED],
        this.programId
      )[0];
    }
    return this._smartWalletSeq;
  }

  get smartWalletSeqData(): Promise<types.SmartWalletSeq> {
    return this.program.account.smartWalletSeq.fetch(this.smartWalletSeq);
  }

  async getLastestSmartWallet(): Promise<anchor.web3.PublicKey> {
    const seqData = await this.program.account.smartWalletSeq.fetch(
      this.smartWalletSeq
    );
    return anchor.web3.PublicKey.findProgramAddressSync(
      [constants.SMART_WALLET_SEED, seqData.seq.toArrayLike(Buffer, 'le', 8)],
      this.programId
    )[0];
  }

  async getSmartWalletConfigData(
    smartWallet: anchor.web3.PublicKey
  ): Promise<types.SmartWalletConfig> {
    return this.program.account.smartWalletConfig.fetch(
      this.smartWalletConfig(smartWallet)
    );
  }

  smartWalletAuthenticator(
    passkey: number[],
    smartWallet: anchor.web3.PublicKey
  ): [anchor.web3.PublicKey, number] {
    const hash = hashSeeds(passkey, smartWallet);
    return anchor.web3.PublicKey.findProgramAddressSync(
      [constants.SMART_WALLET_AUTHENTICATOR_SEED, smartWallet.toBuffer(), hash],
      this.programId
    );
  }

  async getSmartWalletAuthenticatorData(
    smartWalletAuthenticator: anchor.web3.PublicKey
  ): Promise<types.SmartWalletAuthenticator> {
    return this.program.account.smartWalletAuthenticator.fetch(
      smartWalletAuthenticator
    );
  }

  smartWalletConfig(smartWallet: anchor.web3.PublicKey): anchor.web3.PublicKey {
    return anchor.web3.PublicKey.findProgramAddressSync(
      [constants.SMART_WALLET_CONFIG_SEED, smartWallet.toBuffer()],
      this.programId
    )[0];
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

  get config(): anchor.web3.PublicKey {
    if (!this._config) {
      this._config = anchor.web3.PublicKey.findProgramAddressSync(
        [constants.CONFIG_SEED],
        this.programId
      )[0];
    }
    return this._config;
  }

  async initializeTxn(
    payer: anchor.web3.PublicKey
  ): Promise<anchor.web3.Transaction> {
    const ix = await this.program.methods
      .initialize()
      .accountsPartial({
        signer: payer,
        config: this.config,
        smartWalletSeq: this.smartWalletSeq,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .remainingAccounts([
        {
          pubkey: anchor.web3.BPF_LOADER_PROGRAM_ID,
          isWritable: false,
          isSigner: false,
        },
      ])
      .instruction();
    return new anchor.web3.Transaction().add(ix);
  }

  async createSmartWalletTxn(
    passkeyPubkey: number[],
    payer: anchor.web3.PublicKey,
    credentialId: string = '',
    ruleIns: anchor.web3.TransactionInstruction | null = null
  ): Promise<anchor.web3.Transaction> {
    const smartWallet = await this.getLastestSmartWallet();
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
        ruleInstruction.data
      )
      .accountsPartial({
        signer: payer,
        smartWalletSeq: this.smartWalletSeq,
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
    return tx;
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
  ): Promise<anchor.web3.Transaction> {
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

    const txn = new anchor.web3.Transaction()
      .add(verifySignatureIx)
      .add(executeInstructionIx);

    txn.feePayer = payer;
    txn.recentBlockhash = (
      await this.connection.getLatestBlockhash()
    ).blockhash;
    return txn;
  }

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
    smartWalletAuthenticatorString: string,
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
    // - cpi_data (Vec<u8>): 4 bytes length + data bytes

    const currentTimestamp = Math.floor(Date.now() / 1000);

    // Calculate buffer size based on whether rule_data is provided
    const ruleDataLength = ruleInstruction ? ruleInstruction.data.length : 0;
    const ruleDataSize = ruleInstruction ? 5 + ruleDataLength : 1; // 1 byte for Option + 4 bytes length + data (if Some)
    const buffer = Buffer.alloc(
      18 + ruleDataSize + 4 + cpiInstruction.data.length
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

    // Write cpi_data length as little-endian u32
    const cpiDataOffset = 18 + ruleDataSize;
    buffer.writeUInt32LE(cpiInstruction.data.length, cpiDataOffset);

    // Write cpi_data bytes
    cpiInstruction.data.copy(buffer, cpiDataOffset + 4);

    return buffer;
  }
}
