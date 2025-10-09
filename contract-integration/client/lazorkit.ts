import { Program, BN } from '@coral-xyz/anchor';
import {
  PublicKey,
  Transaction,
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
  derivePolicyProgramRegistryPda,
  deriveSmartWalletPda,
  deriveSmartWalletConfigPda,
  deriveWalletDevicePda,
  deriveChunkPda,
  derivePermissionPda,
  deriveLazorkitVaultPda,
} from '../pda/lazorkit';
import {
  getRandomBytes,
  instructionToAccountMetas,
  getVaultIndex,
} from '../utils';
import * as types from '../types';
import { DefaultPolicyClient } from './defaultPolicy';
import * as bs58 from 'bs58';
import {
  buildCallPolicyMessage,
  buildChangePolicyMessage,
  buildExecuteMessage,
  buildCreateChunkMessage,
} from '../messages';
import { Buffer } from 'buffer';
import {
  buildPasskeyVerificationInstruction,
  convertPasskeySignatureToInstructionArgs,
} from '../auth';
import {
  buildTransaction,
  combineInstructionsWithAuth,
  calculateVerifyInstructionIndex,
} from '../transaction';

global.Buffer = Buffer;

Buffer.prototype.subarray = function subarray(
  begin: number | undefined,
  end: number | undefined
) {
  const result = Uint8Array.prototype.subarray.apply(this, [begin, end]);
  Object.setPrototypeOf(result, Buffer.prototype); // Explicitly add the `Buffer` prototype (adds `readUIntLE`!)
  return result;
};

/**
 * Main client for interacting with the LazorKit smart wallet program
 *
 * This client provides both low-level instruction builders and high-level
 * transaction builders for common smart wallet operations.
 */
export class LazorkitClient {
  readonly connection: Connection;
  readonly program: Program<Lazorkit>;
  readonly programId: PublicKey;
  readonly defaultPolicyProgram: DefaultPolicyClient;

  constructor(connection: Connection) {
    this.connection = connection;
    this.program = new Program<Lazorkit>(LazorkitIdl as Lazorkit, {
      connection: connection,
    });
    this.defaultPolicyProgram = new DefaultPolicyClient(connection);
    this.programId = this.program.programId;
  }

  // ============================================================================
  // PDA Derivation Methods
  // ============================================================================

  /**
   * Derives the program configuration PDA
   */
  getConfigPubkey(): PublicKey {
    return deriveConfigPda(this.programId);
  }

  /**
   * Derives the policy program registry PDA
   */
  getPolicyProgramRegistryPubkey(): PublicKey {
    return derivePolicyProgramRegistryPda(this.programId);
  }

  /**
   * Derives the LazorKit vault PDA
   */
  getLazorkitVaultPubkey(index: number): PublicKey {
    return deriveLazorkitVaultPda(this.programId, index);
  }

  /**
   * Derives a smart wallet PDA from wallet ID
   */
  getSmartWalletPubkey(walletId: BN): PublicKey {
    return deriveSmartWalletPda(this.programId, walletId);
  }

  /**
   * Derives the smart wallet data PDA for a given smart wallet
   */
  getWalletStatePubkey(walletId: BN): PublicKey {
    return deriveSmartWalletConfigPda(this.programId, walletId);
  }

  /**
   * Derives a wallet device PDA for a given smart wallet and passkey
   */
  getWalletDevicePubkey(smartWallet: PublicKey, passkey: number[]): PublicKey {
    return deriveWalletDevicePda(this.programId, smartWallet, passkey)[0];
  }

  /**
   * Derives a transaction session PDA for a given smart wallet and nonce
   */
  getChunkPubkey(smartWallet: PublicKey, lastNonce: BN): PublicKey {
    return deriveChunkPda(this.programId, smartWallet, lastNonce);
  }

  /**
   * Derives an ephemeral authorization PDA for a given smart wallet and ephemeral key
   */
  getPermissionPubkey(
    smartWallet: PublicKey,
    ephemeralPublicKey: PublicKey
  ): PublicKey {
    return derivePermissionPda(this.programId, smartWallet, ephemeralPublicKey);
  }

  // ============================================================================
  // Utility Methods
  // ============================================================================

  /**
   * Generates a random wallet ID
   */
  generateWalletId(): BN {
    return new BN(getRandomBytes(8), 'le');
  }

  /**
   * Gets the referral account for a smart wallet
   */
  private async getReferralAccount(walletId: BN): Promise<PublicKey> {
    const smartWalletConfig = await this.getWalletStateData(walletId);
    return smartWalletConfig.referral;
  }

  /**
   * Generates a random vault index (0-31)
   */
  generateVaultIndex(): number {
    return Math.floor(Math.random() * 32);
  }

  /**
   * Calculates split indices for multiple CPI instructions
   */
  private calculateSplitIndex(
    instructions: TransactionInstruction[]
  ): number[] {
    const splitIndex: number[] = [];
    let currentIndex = 0;

    for (let i = 0; i < instructions.length - 1; i++) {
      currentIndex += instructions[i].keys.length + 1; // +1 because the first account is the program_id
      splitIndex.push(currentIndex);
    }

    return splitIndex;
  }

  // ============================================================================
  // Account Data Fetching Methods
  // ============================================================================

  /**
   * Fetches program configuration data
   */
  async getConfigData() {
    return await this.program.account.config.fetch(this.getConfigPubkey());
  }

  /**
   * Fetches smart wallet data for a given smart wallet
   */
  async getWalletStateData(walletId: BN) {
    const pda = this.getWalletStatePubkey(walletId);
    return await this.program.account.walletState.fetch(pda);
  }

  /**
   * Fetches transaction session data for a given transaction session
   */
  async getChunkData(chunk: PublicKey) {
    return await this.program.account.chunk.fetch(chunk);
  }

  /**
   * Fetches permission data for a given permission
   */
  async getPermissionData(permission: PublicKey) {
    return await this.program.account.permission.fetch(permission);
  }

  /**
   * Finds a smart wallet by passkey public key
   */
  async getSmartWalletByPasskey(passkeyPublicKey: number[]): Promise<{
    smartWallet: PublicKey | null;
    walletDevice: PublicKey | null;
  }> {
    const discriminator = LazorkitIdl.accounts.find(
      (a: any) => a.name === 'WalletDevice'
    )!.discriminator;

    const accounts = await this.connection.getProgramAccounts(this.programId, {
      dataSlice: {
        offset: 8 + 1, // offset: DISCRIMINATOR + BUMPS
        length: 33, // length: PASSKEY_PUBLIC_KEY
      },
      filters: [
        { memcmp: { offset: 0, bytes: bs58.encode(discriminator) } },
        { memcmp: { offset: 8 + 1, bytes: bs58.encode(passkeyPublicKey) } },
      ],
    });

    if (accounts.length === 0) {
      return { walletDevice: null, smartWallet: null };
    }

    const walletDeviceData = await this.getWalletDeviceData(accounts[0].pubkey);

    return {
      walletDevice: accounts[0].pubkey,
      smartWallet: walletDeviceData.smartWalletAddress,
    };
  }

  /**
   * Find smart wallet by credential ID
   */
  async getSmartWalletByCredentialId(credentialId: string): Promise<{
    smartWallet: PublicKey | null;
    smartWalletAuthenticator: PublicKey | null;
    passkeyPubkey: string;
  }> {
    const discriminator = LazorkitIdl.accounts.find(
      (a: any) => a.name === 'WalletDevice'
    )!.discriminator;

    // Convert credential_id to base64 buffer
    const credentialIdBuffer = Buffer.from(credentialId, 'base64');

    const accounts = await this.connection.getProgramAccounts(this.programId, {
      dataSlice: {
        offset: 8 + 1 + 33 + 32 + 4, // offset: DISCRIMINATOR + BUMPS + PASSKEY_PUBLIC_KEY + SMART_WALLET_ADDRESS + VECTOR_LENGTH_OFFSET
        length: credentialIdBuffer.length,
      },
      filters: [
        { memcmp: { offset: 0, bytes: bs58.encode(discriminator) } },
        {
          memcmp: {
            offset: 8 + 1 + 33 + 32 + 4, // offset: DISCRIMINATOR + BUMPS + PASSKEY_PUBLIC_KEY + SMART_WALLET_ADDRESS + VECTOR_LENGTH_OFFSET
            bytes: bs58.encode(credentialIdBuffer),
          },
        },
      ],
    });

    if (accounts.length === 0) {
      return {
        smartWalletAuthenticator: null,
        smartWallet: null,
        passkeyPubkey: '',
      };
    }

    const smartWalletData = await this.getWalletDeviceData(accounts[0].pubkey);

    return {
      smartWalletAuthenticator: accounts[0].pubkey,
      smartWallet: smartWalletData.smartWalletAddress,
      passkeyPubkey: bs58.encode(smartWalletData.passkeyPublicKey),
    };
  }

  // ============================================================================
  // Low-Level Instruction Builders
  // ============================================================================

  /**
   * Builds the initialize program instruction
   */
  async buildInitializeProgramIns(
    payer: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .initializeProgram()
      .accountsPartial({
        signer: payer,
        config: this.getConfigPubkey(),
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        defaultPolicyProgram: this.defaultPolicyProgram.programId,
        systemProgram: SystemProgram.programId,
      })
      .instruction();
  }

  /**
   * Builds the create smart wallet instruction
   */
  async buildCreateSmartWalletIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    walletId: BN,
    policyInstruction: TransactionInstruction,
    args: types.CreateSmartWalletArgs
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .createSmartWallet(args)
      .accountsPartial({
        payer,
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        smartWallet,
        smartWalletState: this.getWalletStatePubkey(walletId),
        config: this.getConfigPubkey(),
        defaultPolicyProgram: this.defaultPolicyProgram.programId,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([
        ...instructionToAccountMetas(policyInstruction, [payer]),
      ])
      .instruction();
  }

  /**
   * Builds the execute direct transaction instruction
   */
  async buildExecuteIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    smartWalletId: BN,
    args: types.ExecuteArgs,
    policyInstruction: TransactionInstruction,
    cpiInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .execute(args)
      .accountsPartial({
        payer,
        smartWallet,
        walletState: this.getWalletStatePubkey(smartWalletId),
        referral: await this.getReferralAccount(smartWalletId),
        lazorkitVault: this.getLazorkitVaultPubkey(args.vaultIndex),
        walletSigner: this.getWalletDevicePubkey(
          smartWallet,
          args.passkeyPublicKey
        ),
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        policyProgram: policyInstruction.programId,
        cpiProgram: cpiInstruction.programId,
        config: this.getConfigPubkey(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([
        ...instructionToAccountMetas(policyInstruction),
        ...instructionToAccountMetas(cpiInstruction, [payer]),
      ])
      .instruction();
  }

  /**
   * Builds the invoke wallet policy instruction
   */
  async buildCallPolicyIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.CallPolicyArgs,
    policyInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    const remaining: AccountMeta[] = [];

    if (args.newWalletDevice) {
      const newWalletDevice = this.getWalletDevicePubkey(
        smartWallet,
        args.newWalletDevice.passkeyPublicKey
      );

      remaining.push({
        pubkey: newWalletDevice,
        isWritable: true,
        isSigner: false,
      });
    }

    remaining.push(...instructionToAccountMetas(policyInstruction));

    return await this.program.methods
      .callPolicy(args)
      .accountsPartial({
        payer,
        config: this.getConfigPubkey(),
        smartWallet,
        smartWalletConfig: this.getWalletStatePubkey(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.getLazorkitVaultPubkey(args.vaultIndex),
        walletDevice: this.getWalletDevicePubkey(
          smartWallet,
          args.passkeyPublicKey
        ),
        policyProgram: policyInstruction.programId,
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  /**
   * Builds the update wallet policy instruction
   */
  async buildChangeRuleIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.ChangePolicyArgs,
    destroyPolicyInstruction: TransactionInstruction,
    initPolicyInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    const remaining: AccountMeta[] = [];

    if (args.newWalletDevice) {
      const newWalletDevice = this.getWalletDevicePubkey(
        smartWallet,
        args.newWalletDevice.passkeyPublicKey
      );
      remaining.push({
        pubkey: newWalletDevice,
        isWritable: true,
        isSigner: false,
      });
    }

    remaining.push(...instructionToAccountMetas(destroyPolicyInstruction));
    remaining.push(...instructionToAccountMetas(initPolicyInstruction));

    return await this.program.methods
      .changePolicy(args)
      .accountsPartial({
        payer,
        config: this.getConfigPubkey(),
        smartWallet,
        smartWalletConfig: this.getWalletStatePubkey(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.getLazorkitVaultPubkey(args.vaultIndex),
        walletDevice: this.getWalletDevicePubkey(
          smartWallet,
          args.passkeyPublicKey
        ),
        oldPolicyProgram: destroyPolicyInstruction.programId,
        newPolicyProgram: initPolicyInstruction.programId,
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  /**
   * Builds the create deferred execution instruction
   */
  async buildCreateChunkIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.CreateChunkArgs,
    policyInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .createChunk(args)
      .accountsPartial({
        payer,
        config: this.getConfigPubkey(),
        smartWallet,
        smartWalletConfig: this.getWalletStatePubkey(smartWallet),
        walletDevice: this.getWalletDevicePubkey(
          smartWallet,
          args.passkeyPublicKey
        ),
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        policyProgram: policyInstruction.programId,
        chunk: this.getChunkPubkey(
          smartWallet,
          await this.getWalletStateData(smartWallet).then((d) => d.lastNonce)
        ),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([...instructionToAccountMetas(policyInstruction)])
      .instruction();
  }

  /**
   * Builds the execute deferred transaction instruction
   */
  async buildExecuteChunkIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    cpiInstructions: TransactionInstruction[]
  ): Promise<TransactionInstruction> {
    const cfg = await this.getWalletStateData(smartWallet);
    const chunk = this.getChunkPubkey(
      smartWallet,
      cfg.lastNonce.sub(new BN(1))
    );

    const chunkData = await this.getChunkData(chunk);

    // Prepare CPI data and split indices
    const instructionDataList = cpiInstructions.map((ix) =>
      Buffer.from(Array.from(ix.data))
    );
    const splitIndex = this.calculateSplitIndex(cpiInstructions);

    // Combine all account metas from all instructions
    const allAccountMetas = cpiInstructions.flatMap((ix) => [
      {
        pubkey: ix.programId,
        isSigner: false,
        isWritable: false,
      },
      ...instructionToAccountMetas(ix, [payer]),
    ]);

    return await this.program.methods
      .executeChunk(instructionDataList, Buffer.from(splitIndex))
      .accountsPartial({
        payer,
        config: this.getConfigPubkey(),
        smartWallet,
        smartWalletConfig: this.getWalletStatePubkey(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.getLazorkitVaultPubkey(chunkData.vaultIndex), // Will be updated based on session
        chunk,
        sessionRefund: chunkData.rentRefundAddress,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(allAccountMetas)
      .instruction();
  }

  /**
   * Builds the close chunk instruction
   */
  async buildCloseChunkIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    nonce: BN
  ): Promise<TransactionInstruction> {
    const chunk = this.getChunkPubkey(smartWallet, nonce);

    const sessionRefund = await this.getChunkData(chunk).then(
      (d) => d.rentRefundAddress
    );

    const smartWalletConfig = this.getWalletStatePubkey(smartWallet);

    return await this.program.methods
      .closeChunk()
      .accountsPartial({
        payer,
        smartWallet,
        smartWalletConfig,
        chunk,
        sessionRefund,
      })
      .instruction();
  }

  /**
   * Builds the authorize ephemeral execution instruction
   */
  async buildGrantPermissionIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.GrantPermissionArgs,
    cpiInstructions: TransactionInstruction[]
  ): Promise<TransactionInstruction> {
    // Combine all account metas from all instructions
    const allAccountMetas = cpiInstructions.flatMap((ix) =>
      instructionToAccountMetas(ix, [payer])
    );

    return await this.program.methods
      .grantPermission(args)
      .accountsPartial({
        payer,
        config: this.getConfigPubkey(),
        smartWallet,
        smartWalletConfig: this.getWalletStatePubkey(smartWallet),
        walletDevice: this.getWalletDevicePubkey(
          smartWallet,
          args.passkeyPublicKey
        ),
        permission: this.getPermissionPubkey(
          smartWallet,
          args.ephemeralPublicKey
        ),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(allAccountMetas)
      .instruction();
  }

  /**
   * Builds the execute ephemeral authorization instruction
   */
  async buildExecuteWithPermissionIns(
    feePayer: PublicKey,
    ephemeralSigner: PublicKey,
    smartWallet: PublicKey,
    permission: PublicKey,
    cpiInstructions: TransactionInstruction[]
  ): Promise<TransactionInstruction> {
    // Prepare CPI data and split indices
    const instructionDataList = cpiInstructions.map((ix) =>
      Array.from(ix.data)
    );
    const splitIndex = this.calculateSplitIndex(cpiInstructions);

    const vaultIndex = await this.getPermissionData(permission).then(
      (d) => d.vaultIndex
    );

    // Combine all account metas from all instructions
    const allAccountMetas = cpiInstructions.flatMap((ix) =>
      instructionToAccountMetas(ix, [feePayer])
    );

    return await this.program.methods
      .executeWithPermission(
        instructionDataList.map((data) => Buffer.from(data)),
        Buffer.from(splitIndex)
      )
      .accountsPartial({
        feePayer,
        ephemeralSigner,
        config: this.getConfigPubkey(),
        smartWallet,
        smartWalletConfig: this.getWalletStatePubkey(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.getLazorkitVaultPubkey(vaultIndex), // Will be updated based on authorization
        permission,
        authorizationRefund: feePayer,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(allAccountMetas)
      .instruction();
  }

  // ============================================================================
  // High-Level Transaction Builders (with Authentication)
  // ============================================================================

  async manageVaultTxn(
    params: types.ManageVaultParams,
    options: types.TransactionBuilderOptions = {}
  ): Promise<Transaction | VersionedTransaction> {
    const manageVaultInstruction = await this.program.methods
      .manageVault(
        params.action === 'deposit' ? 0 : 1,
        params.amount,
        params.vaultIndex
      )
      .accountsPartial({
        authority: params.payer,
        config: this.getConfigPubkey(),
        vault: this.getLazorkitVaultPubkey(params.vaultIndex),
        destination: params.destination,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    const result = await buildTransaction(
      this.connection,
      params.payer,
      [manageVaultInstruction],
      options
    );

    return result.transaction;
  }

  /**
   * Creates a smart wallet with passkey authentication
   */
  async createSmartWalletTxn(
    params: types.CreateSmartWalletParams,
    options: types.TransactionBuilderOptions = {}
  ): Promise<{
    transaction: Transaction | VersionedTransaction;
    smartWalletId: BN;
    smartWallet: PublicKey;
  }> {
    const smartWalletId = params.smartWalletId || this.generateWalletId();
    const smartWallet = this.getSmartWalletPubkey(smartWalletId);
    const walletState = this.getWalletStatePubkey(smartWalletId);

    const credentialId = Buffer.from(params.credentialIdBase64, 'base64');
    const credentialHash = Array.from(
      new Uint8Array(require('js-sha256').arrayBuffer(credentialId))
    );

    let policyInstruction = await this.defaultPolicyProgram.buildInitPolicyIx(
      params.smartWalletId,
      params.passkeyPublicKey,
      credentialHash,
      smartWallet,
      walletState
    );

    if (params.policyInstruction) {
      policyInstruction = params.policyInstruction;
    }

    const args = {
      passkeyPublicKey: params.passkeyPublicKey,
      credentialHash,
      initPolicyData: policyInstruction.data,
      walletId: smartWalletId,
      amount: params.amount,
      referralAddress: params.referral_address || null,
      vaultIndex: getVaultIndex(params.vaultIndex, () =>
        this.generateVaultIndex()
      ),
    };

    const instruction = await this.buildCreateSmartWalletIns(
      params.payer,
      smartWallet,
      smartWalletId,
      policyInstruction,
      args
    );

    const result = await buildTransaction(
      this.connection,
      params.payer,
      [instruction],
      options
    );
    const transaction = result.transaction;

    return {
      transaction,
      smartWalletId,
      smartWallet,
    };
  }

  /**
   * Executes a direct transaction with passkey authentication
   */
  async executeTxn(
    params: types.ExecuteParams,
    options: types.TransactionBuilderOptions = {}
  ): Promise<Transaction | VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const walletStateData = await this.getWalletStateData(params.smartWalletId);

    const smartWalletId = walletStateData.walletId;

    const credentialHash = walletStateData.devices.find((device) =>
      device.passkeyPubkey.every(
        (byte, index) =>
          byte === params.passkeySignature.passkeyPublicKey[index]
      )
    )?.credentialHash;

    const walletDevice = this.getWalletDevicePubkey(
      params.smartWallet,
      params.passkeySignature.passkeyPublicKey
    );

    console.log('walletDevice', walletDevice.toString());

    let policyInstruction = await this.defaultPolicyProgram.buildCheckPolicyIx(
      smartWalletId,
      params.passkeySignature.passkeyPublicKey,
      walletDevice,
      params.smartWallet,
      credentialHash,
      walletStateData.policyData
    );

    if (params.policyInstruction) {
      policyInstruction = params.policyInstruction;
    }

    const signatureArgs = convertPasskeySignatureToInstructionArgs(
      params.passkeySignature
    );

    const execInstruction = await this.buildExecuteIns(
      params.payer,
      params.smartWallet,
      smartWalletId,
      {
        ...signatureArgs,
        verifyInstructionIndex: calculateVerifyInstructionIndex(
          options.computeUnitLimit
        ),
        splitIndex: policyInstruction.keys.length,
        policyData: policyInstruction.data,
        cpiData: params.cpiInstruction.data,
        timestamp: params.timestamp,
        vaultIndex:
          params.vaultIndex !== undefined
            ? params.vaultIndex
            : this.generateVaultIndex(),
      },
      policyInstruction,
      params.cpiInstruction
    );

    console.log(1);

    const instructions = combineInstructionsWithAuth(authInstruction, [
      execInstruction,
    ]);

    const result = await buildTransaction(
      this.connection,
      params.payer,
      instructions,
      options
    );

    return result.transaction;
  }

  /**
   * Invokes a wallet policy with passkey authentication
   */
  async callPolicyTxn(
    params: types.CallPolicyParams,
    options: types.TransactionBuilderOptions = {}
  ): Promise<Transaction | VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const signatureArgs = convertPasskeySignatureToInstructionArgs(
      params.passkeySignature
    );

    const invokeInstruction = await this.buildCallPolicyIns(
      params.payer,
      params.smartWallet,
      {
        ...signatureArgs,
        newWalletDevice: params.newWalletDevice
          ? {
              passkeyPublicKey: Array.from(
                params.newWalletDevice.passkeyPublicKey
              ),
              credentialId: Buffer.from(
                params.newWalletDevice.credentialIdBase64,
                'base64'
              ),
            }
          : null,
        policyData: params.policyInstruction.data,
        verifyInstructionIndex: calculateVerifyInstructionIndex(
          options.computeUnitLimit
        ),
        timestamp: params.timestamp,
        vaultIndex: getVaultIndex(params.vaultIndex, () =>
          this.generateVaultIndex()
        ),
        smartWalletIsSigner: params.smartWalletIsSigner === true,
      },
      params.policyInstruction
    );

    const instructions = combineInstructionsWithAuth(authInstruction, [
      invokeInstruction,
    ]);

    const result = await buildTransaction(
      this.connection,
      params.payer,
      instructions,
      options
    );

    return result.transaction;
  }

  /**
   * Updates a wallet policy with passkey authentication
   */
  async changePolicyTxn(
    params: types.ChangePolicyParams,
    options: types.TransactionBuilderOptions = {}
  ): Promise<Transaction | VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const signatureArgs = convertPasskeySignatureToInstructionArgs(
      params.passkeySignature
    );

    const updateInstruction = await this.buildChangeRuleIns(
      params.payer,
      params.smartWallet,
      {
        ...signatureArgs,
        verifyInstructionIndex: calculateVerifyInstructionIndex(
          options.computeUnitLimit
        ),
        destroyPolicyData: params.destroyPolicyInstruction.data,
        initPolicyData: params.initPolicyInstruction.data,
        splitIndex:
          (params.newWalletDevice ? 1 : 0) +
          params.destroyPolicyInstruction.keys.length,
        newWalletDevice: params.newWalletDevice
          ? {
              passkeyPublicKey: Array.from(
                params.newWalletDevice.passkeyPublicKey
              ),
              credentialId: Buffer.from(
                params.newWalletDevice.credentialIdBase64,
                'base64'
              ),
            }
          : null,
        timestamp: new BN(Math.floor(Date.now() / 1000)),
        vaultIndex: getVaultIndex(params.vaultIndex, () =>
          this.generateVaultIndex()
        ),
      },
      params.destroyPolicyInstruction,
      params.initPolicyInstruction
    );

    const instructions = combineInstructionsWithAuth(authInstruction, [
      updateInstruction,
    ]);

    const result = await buildTransaction(
      this.connection,
      params.payer,
      instructions,
      options
    );

    return result.transaction;
  }

  /**
   * Creates a deferred execution with passkey authentication
   */
  async createChunkTxn(
    params: types.CreateChunkParams,
    options: types.TransactionBuilderOptions = {}
  ): Promise<Transaction | VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const smartWalletId = await this.getWalletStateData(
      params.smartWallet
    ).then((d) => d.walletId);

    let policyInstruction = await this.defaultPolicyProgram.buildCheckPolicyIx(
      smartWalletId,
      params.passkeySignature.passkeyPublicKey,
      this.getWalletDevicePubkey(
        params.smartWallet,
        params.passkeySignature.passkeyPublicKey
      ),
      params.smartWallet
    );

    if (params.policyInstruction) {
      policyInstruction = params.policyInstruction;
    }

    const signatureArgs = convertPasskeySignatureToInstructionArgs(
      params.passkeySignature
    );

    // Calculate cpiHash from empty CPI instructions (since create chunk doesn't have CPI instructions)
    const { computeMultipleCpiHashes } = await import('../messages');
    const cpiHashes = computeMultipleCpiHashes(
      params.payer,
      params.cpiInstructions,
      params.smartWallet
    );

    // Create combined hash of CPI hashes
    const cpiCombined = new Uint8Array(64); // 32 + 32 bytes
    cpiCombined.set(cpiHashes.cpiDataHash, 0);
    cpiCombined.set(cpiHashes.cpiAccountsHash, 32);
    const cpiHash = new Uint8Array(
      require('js-sha256').arrayBuffer(cpiCombined)
    );

    const sessionInstruction = await this.buildCreateChunkIns(
      params.payer,
      params.smartWallet,
      {
        ...signatureArgs,
        policyData: policyInstruction?.data || Buffer.alloc(0),
        verifyInstructionIndex: calculateVerifyInstructionIndex(
          options.computeUnitLimit
        ),
        timestamp: params.timestamp || new BN(Math.floor(Date.now() / 1000)),
        cpiHash: Array.from(cpiHash),
        vaultIndex: getVaultIndex(params.vaultIndex, () =>
          this.generateVaultIndex()
        ),
      },
      policyInstruction
    );

    const instructions = combineInstructionsWithAuth(authInstruction, [
      sessionInstruction,
    ]);

    const result = await buildTransaction(
      this.connection,
      params.payer,
      instructions,
      options
    );

    return result.transaction;
  }

  /**
   * Executes a deferred transaction (no authentication needed)
   */
  async executeChunkTxn(
    params: types.ExecuteChunkParams,
    options: types.TransactionBuilderOptions = {}
  ): Promise<Transaction | VersionedTransaction> {
    const instruction = await this.buildExecuteChunkIns(
      params.payer,
      params.smartWallet,
      params.cpiInstructions
    );

    const result = await buildTransaction(
      this.connection,
      params.payer,
      [instruction],
      options
    );

    return result.transaction;
  }

  /**
   * Closes a deferred transaction (no authentication needed)
   */
  async closeChunkTxn(
    params: types.CloseChunkParams,
    options: types.TransactionBuilderOptions = {}
  ): Promise<Transaction | VersionedTransaction> {
    const instruction = await this.buildCloseChunkIns(
      params.payer,
      params.smartWallet,
      params.nonce
    );

    const result = await buildTransaction(
      this.connection,
      params.payer,
      [instruction],
      options
    );

    return result.transaction;
  }

  // ============================================================================
  // Message Building Methods
  // ============================================================================

  /**
   * Builds an authorization message for a smart wallet action
   */
  async buildAuthorizationMessage(params: {
    action: types.SmartWalletActionArgs;
    payer: PublicKey;
    smartWallet: PublicKey;
    passkeyPublicKey: number[];
  }): Promise<Buffer> {
    let message: Buffer;
    const { action, payer, smartWallet, passkeyPublicKey } = params;

    switch (action.type) {
      case types.SmartWalletAction.Execute: {
        const { policyInstruction: policyIns, cpiInstruction } =
          action.args as types.ArgsByAction[types.SmartWalletAction.Execute];

        const smartWalletId = await this.getWalletStateData(smartWallet).then(
          (d) => d.walletId
        );

        let policyInstruction =
          await this.defaultPolicyProgram.buildCheckPolicyIx(
            smartWalletId,
            passkeyPublicKey,
            this.getWalletDevicePubkey(smartWallet, passkeyPublicKey),
            params.smartWallet
          );

        if (policyIns) {
          policyInstruction = policyIns;
        }

        const smartWalletConfig = await this.getWalletStateData(smartWallet);

        const timestamp = new BN(Math.floor(Date.now() / 1000));
        message = buildExecuteMessage(
          payer,
          smartWallet,
          smartWalletConfig.lastNonce,
          timestamp,
          policyInstruction,
          cpiInstruction
        );
        break;
      }
      case types.SmartWalletAction.CallPolicy: {
        const { policyInstruction } =
          action.args as types.ArgsByAction[types.SmartWalletAction.CallPolicy];

        const smartWalletConfig = await this.getWalletStateData(smartWallet);

        const timestamp = new BN(Math.floor(Date.now() / 1000));
        message = buildCallPolicyMessage(
          payer,
          smartWallet,
          smartWalletConfig.lastNonce,
          timestamp,
          policyInstruction
        );
        break;
      }
      case types.SmartWalletAction.ChangePolicy: {
        const { initPolicyIns, destroyPolicyIns } =
          action.args as types.ArgsByAction[types.SmartWalletAction.ChangePolicy];

        const smartWalletConfig = await this.getWalletStateData(smartWallet);

        const timestamp = new BN(Math.floor(Date.now() / 1000));
        message = buildChangePolicyMessage(
          payer,
          smartWallet,
          smartWalletConfig.lastNonce,
          timestamp,
          destroyPolicyIns,
          initPolicyIns
        );
        break;
      }
      case types.SmartWalletAction.CreateChunk: {
        const { policyInstruction, cpiInstructions, expiresAt } =
          action.args as types.ArgsByAction[types.SmartWalletAction.CreateChunk];

        const smartWalletConfig = await this.getWalletStateData(smartWallet);

        const timestamp = new BN(Math.floor(Date.now() / 1000));
        message = buildCreateChunkMessage(
          payer,
          smartWallet,
          smartWalletConfig.lastNonce,
          timestamp,
          policyInstruction,
          cpiInstructions
        );
        break;
      }

      default:
        throw new Error(`Unsupported SmartWalletAction: ${action.type}`);
    }

    return message;
  }
}
