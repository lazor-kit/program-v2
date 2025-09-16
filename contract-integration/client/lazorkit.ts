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
  deriveProgramConfigPda,
  derivePolicyProgramRegistryPda,
  deriveSmartWalletPda,
  deriveSmartWalletDataPda,
  deriveWalletDevicePda,
  deriveTransactionSessionPda,
  deriveEphemeralAuthorizationPda,
  deriveLazorkitVaultPda,
} from '../pda/lazorkit';
import { getRandomBytes, instructionToAccountMetas } from '../utils';
import * as types from '../types';
import { DefaultPolicyClient } from './defaultPolicy';
import * as bs58 from 'bs58';
import {
  buildInvokePolicyMessage,
  buildUpdatePolicyMessage,
  buildExecuteMessage,
} from '../messages';
import { Buffer } from 'buffer';
import {
  buildPasskeyVerificationInstruction,
  convertPasskeySignatureToInstructionArgs,
} from '../auth';
import {
  buildVersionedTransaction,
  buildLegacyTransaction,
  combineInstructionsWithAuth,
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
  programConfigPda(): PublicKey {
    return deriveProgramConfigPda(this.programId);
  }

  /**
   * Derives the policy program registry PDA
   */
  policyProgramRegistryPda(): PublicKey {
    return derivePolicyProgramRegistryPda(this.programId);
  }

  /**
   * Derives the LazorKit vault PDA
   */
  lazorkitVaultPda(index: number): PublicKey {
    return deriveLazorkitVaultPda(this.programId, index);
  }

  /**
   * Derives a smart wallet PDA from wallet ID
   */
  smartWalletPda(walletId: BN): PublicKey {
    return deriveSmartWalletPda(this.programId, walletId);
  }

  /**
   * Derives the smart wallet data PDA for a given smart wallet
   */
  smartWalletDataPda(smartWallet: PublicKey): PublicKey {
    return deriveSmartWalletDataPda(this.programId, smartWallet);
  }

  /**
   * Derives a wallet device PDA for a given smart wallet and passkey
   */
  walletDevicePda(smartWallet: PublicKey, passkey: number[]): PublicKey {
    return deriveWalletDevicePda(this.programId, smartWallet, passkey)[0];
  }

  /**
   * Derives a transaction session PDA for a given smart wallet and nonce
   */
  transactionSessionPda(smartWallet: PublicKey, lastNonce: BN): PublicKey {
    return deriveTransactionSessionPda(this.programId, smartWallet, lastNonce);
  }

  /**
   * Derives an ephemeral authorization PDA for a given smart wallet and ephemeral key
   */
  ephemeralAuthorizationPda(
    smartWallet: PublicKey,
    ephemeralPublicKey: PublicKey
  ): PublicKey {
    return deriveEphemeralAuthorizationPda(
      this.programId,
      smartWallet,
      ephemeralPublicKey
    );
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
  private async getReferralAccount(smartWallet: PublicKey): Promise<PublicKey> {
    const smartWalletData = await this.getSmartWalletData(smartWallet);
    return smartWalletData.referralAddress;
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
    return await this.program.account.programConfig.fetch(
      this.programConfigPda()
    );
  }

  /**
   * Fetches smart wallet data for a given smart wallet
   */
  async getSmartWalletData(smartWallet: PublicKey) {
    const pda = this.smartWalletDataPda(smartWallet);
    return await this.program.account.smartWalletData.fetch(pda);
  }

  /**
   * Fetches wallet device data for a given device
   */
  async getWalletDeviceData(walletDevice: PublicKey) {
    return await this.program.account.walletDevice.fetch(walletDevice);
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
        offset: 8,
        length: 33,
      },
      filters: [
        { memcmp: { offset: 0, bytes: bs58.encode(discriminator) } },
        { memcmp: { offset: 8, bytes: bs58.encode(passkeyPublicKey) } },
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

  // ============================================================================
  // Low-Level Instruction Builders
  // ============================================================================

  /**
   * Builds the initialize program instruction
   */
  async buildInitializeProgramInstruction(
    payer: PublicKey
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .initializeProgram()
      .accountsPartial({
        signer: payer,
        config: this.programConfigPda(),
        policyProgramRegistry: this.policyProgramRegistryPda(),
        defaultPolicyProgram: this.defaultPolicyProgram.programId,
        systemProgram: SystemProgram.programId,
      })
      .instruction();
  }

  /**
   * Builds the create smart wallet instruction
   */
  async buildCreateSmartWalletInstruction(
    payer: PublicKey,
    smartWallet: PublicKey,
    walletDevice: PublicKey,
    policyInstruction: TransactionInstruction,
    args: types.CreateSmartWalletArgs
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .createSmartWallet(args)
      .accountsPartial({
        payer,
        policyProgramRegistry: this.policyProgramRegistryPda(),
        smartWallet,
        smartWalletData: this.smartWalletDataPda(smartWallet),
        walletDevice,
        config: this.programConfigPda(),
        defaultPolicyProgram: this.defaultPolicyProgram.programId,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([...instructionToAccountMetas(policyInstruction)])
      .instruction();
  }

  /**
   * Builds the execute direct transaction instruction
   */
  async buildExecuteDirectTransactionInstruction(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.ExecuteDirectTransactionArgs,
    policyInstruction: TransactionInstruction,
    cpiInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .executeDirectTransaction(args)
      .accountsPartial({
        payer,
        smartWallet,
        smartWalletData: this.smartWalletDataPda(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.lazorkitVaultPda(args.vaultIndex),
        walletDevice: this.walletDevicePda(smartWallet, args.passkeyPublicKey),
        policyProgramRegistry: this.policyProgramRegistryPda(),
        policyProgram: policyInstruction.programId,
        cpiProgram: cpiInstruction.programId,
        config: this.programConfigPda(),
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
  async buildInvokeWalletPolicyInstruction(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.InvokeWalletPolicyArgs,
    policyInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    const remaining: AccountMeta[] = [];

    if (args.newWalletDevice) {
      const newWalletDevice = this.walletDevicePda(
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
      .invokeWalletPolicy(args)
      .accountsPartial({
        payer,
        config: this.programConfigPda(),
        smartWallet,
        smartWalletData: this.smartWalletDataPda(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.lazorkitVaultPda(args.vaultIndex),
        walletDevice: this.walletDevicePda(smartWallet, args.passkeyPublicKey),
        policyProgram: policyInstruction.programId,
        policyProgramRegistry: this.policyProgramRegistryPda(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  /**
   * Builds the update wallet policy instruction
   */
  async buildUpdateWalletPolicyInstruction(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.UpdateWalletPolicyArgs,
    destroyPolicyInstruction: TransactionInstruction,
    initPolicyInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    const remaining: AccountMeta[] = [];

    if (args.newWalletDevice) {
      const newWalletDevice = this.walletDevicePda(
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
      .updateWalletPolicy(args)
      .accountsPartial({
        payer,
        config: this.programConfigPda(),
        smartWallet,
        smartWalletData: this.smartWalletDataPda(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.lazorkitVaultPda(args.vaultIndex),
        walletDevice: this.walletDevicePda(smartWallet, args.passkeyPublicKey),
        oldPolicyProgram: destroyPolicyInstruction.programId,
        newPolicyProgram: initPolicyInstruction.programId,
        policyProgramRegistry: this.policyProgramRegistryPda(),
        ixSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(remaining)
      .instruction();
  }

  /**
   * Builds the create deferred execution instruction
   */
  async buildCreateDeferredExecutionInstruction(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.CreateDeferredExecutionArgs,
    policyInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .createDeferredExecution(args)
      .accountsPartial({
        payer,
        config: this.programConfigPda(),
        smartWallet,
        smartWalletData: this.smartWalletDataPda(smartWallet),
        walletDevice: this.walletDevicePda(smartWallet, args.passkeyPublicKey),
        policyProgramRegistry: this.policyProgramRegistryPda(),
        policyProgram: policyInstruction.programId,
        transactionSession: this.transactionSessionPda(
          smartWallet,
          await this.getSmartWalletData(smartWallet).then((d) => d.lastNonce)
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
  async buildExecuteDeferredTransactionInstruction(
    payer: PublicKey,
    smartWallet: PublicKey,
    cpiInstructions: TransactionInstruction[],
    vaultIndex: number
  ): Promise<TransactionInstruction> {
    const cfg = await this.getSmartWalletData(smartWallet);
    const transactionSession = this.transactionSessionPda(
      smartWallet,
      cfg.lastNonce
    );

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
      .executeDeferredTransaction(
        instructionDataList,
        Buffer.from(splitIndex),
        vaultIndex
      )
      .accountsPartial({
        payer,
        config: this.programConfigPda(),
        smartWallet,
        smartWalletData: this.smartWalletDataPda(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.lazorkitVaultPda(vaultIndex), // Will be updated based on session
        transactionSession,
        sessionRefund: payer,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(allAccountMetas)
      .instruction();
  }

  /**
   * Builds the authorize ephemeral execution instruction
   */
  async buildAuthorizeEphemeralExecutionInstruction(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.AuthorizeEphemeralExecutionArgs,
    cpiInstructions: TransactionInstruction[]
  ): Promise<TransactionInstruction> {
    // Combine all account metas from all instructions
    const allAccountMetas = cpiInstructions.flatMap((ix) =>
      instructionToAccountMetas(ix, [payer])
    );

    return await this.program.methods
      .authorizeEphemeralExecution(args)
      .accountsPartial({
        payer,
        config: this.programConfigPda(),
        smartWallet,
        smartWalletData: this.smartWalletDataPda(smartWallet),
        walletDevice: this.walletDevicePda(smartWallet, args.passkeyPublicKey),
        ephemeralAuthorization: this.ephemeralAuthorizationPda(
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
  async buildExecuteEphemeralAuthorizationInstruction(
    feePayer: PublicKey,
    ephemeralSigner: PublicKey,
    smartWallet: PublicKey,
    ephemeralAuthorization: PublicKey,
    cpiInstructions: TransactionInstruction[],
    vaultIndex: number
  ): Promise<TransactionInstruction> {
    // Prepare CPI data and split indices
    const instructionDataList = cpiInstructions.map((ix) =>
      Array.from(ix.data)
    );
    const splitIndex = this.calculateSplitIndex(cpiInstructions);

    // Combine all account metas from all instructions
    const allAccountMetas = cpiInstructions.flatMap((ix) =>
      instructionToAccountMetas(ix, [feePayer])
    );

    return await this.program.methods
      .executeEphemeralAuthorization(
        instructionDataList.map((data) => Buffer.from(data)),
        Buffer.from(splitIndex),
        vaultIndex
      )
      .accountsPartial({
        feePayer,
        ephemeralSigner,
        config: this.programConfigPda(),
        smartWallet,
        smartWalletData: this.smartWalletDataPda(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.lazorkitVaultPda(0), // Will be updated based on authorization
        ephemeralAuthorization,
        authorizationRefund: feePayer,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(allAccountMetas)
      .instruction();
  }

  // ============================================================================
  // High-Level Transaction Builders (with Authentication)
  // ============================================================================

  async createManageVaultTransaction(
    params: types.ManageVaultParams
  ): Promise<VersionedTransaction> {
    const manageVaultInstruction = await this.program.methods
      .manageVault(
        params.action === 'deposit' ? 0 : 1,
        params.amount,
        params.vaultIndex
      )
      .accountsPartial({
        authority: params.payer,
        config: this.programConfigPda(),
        vault: this.lazorkitVaultPda(params.vaultIndex),
        destination: params.destination,
        systemProgram: SystemProgram.programId,
      })
      .instruction();
    return buildVersionedTransaction(this.connection, params.payer, [
      manageVaultInstruction,
    ]);
  }

  /**
   * Creates a smart wallet with passkey authentication
   */
  async createSmartWalletTransaction(
    params: types.CreateSmartWalletParams
  ): Promise<{
    transaction: Transaction;
    smartWalletId: BN;
    smartWallet: PublicKey;
  }> {
    const smartWalletId = params.smartWalletId || this.generateWalletId();
    const smartWallet = this.smartWalletPda(smartWalletId);
    const walletDevice = this.walletDevicePda(
      smartWallet,
      params.passkeyPublicKey
    );

    let policyInstruction = await this.defaultPolicyProgram.buildInitPolicyIx(
      params.smartWalletId,
      params.passkeyPublicKey,
      smartWallet,
      walletDevice
    );

    if (params.policyInstruction) {
      policyInstruction = params.policyInstruction;
    }

    const args = {
      passkeyPublicKey: params.passkeyPublicKey,
      credentialId: Buffer.from(params.credentialIdBase64, 'base64'),
      policyData: policyInstruction.data,
      walletId: smartWalletId,
      amount: params.amount,
      referralAddress: params.referral_address || null,
      vaultIndex: params.vaultIndex || this.generateVaultIndex(),
    };

    const instruction = await this.buildCreateSmartWalletInstruction(
      params.payer,
      smartWallet,
      walletDevice,
      policyInstruction,
      args
    );

    const transaction = await buildLegacyTransaction(
      this.connection,
      params.payer,
      [instruction]
    );

    return {
      transaction,
      smartWalletId,
      smartWallet,
    };
  }

  /**
   * Executes a direct transaction with passkey authentication
   */
  async createExecuteDirectTransaction(
    params: types.ExecuteDirectTransactionParams
  ): Promise<VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const smartWalletId = await this.getSmartWalletData(
      params.smartWallet
    ).then((d) => d.walletId);

    let policyInstruction = await this.defaultPolicyProgram.buildCheckPolicyIx(
      smartWalletId,
      params.passkeySignature.passkeyPublicKey,
      this.walletDevicePda(
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

    const execInstruction = await this.buildExecuteDirectTransactionInstruction(
      params.payer,
      params.smartWallet,
      {
        ...signatureArgs,
        verifyInstructionIndex: 0,
        splitIndex: policyInstruction.keys.length,
        policyData: policyInstruction.data,
        cpiData: params.cpiInstruction.data,
        vaultIndex: params.vaultIndex || this.generateVaultIndex(),
      },
      policyInstruction,
      params.cpiInstruction
    );

    const instructions = combineInstructionsWithAuth(authInstruction, [
      execInstruction,
    ]);
    return buildVersionedTransaction(
      this.connection,
      params.payer,
      instructions
    );
  }

  /**
   * Invokes a wallet policy with passkey authentication
   */
  async createInvokeWalletPolicyTransaction(
    params: types.InvokeWalletPolicyParams
  ): Promise<VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const signatureArgs = convertPasskeySignatureToInstructionArgs(
      params.passkeySignature
    );

    const invokeInstruction = await this.buildInvokeWalletPolicyInstruction(
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
        verifyInstructionIndex: 0,
        vaultIndex: params.vaultIndex || this.generateVaultIndex(),
      },
      params.policyInstruction
    );

    const instructions = combineInstructionsWithAuth(authInstruction, [
      invokeInstruction,
    ]);
    return buildVersionedTransaction(
      this.connection,
      params.payer,
      instructions
    );
  }

  /**
   * Updates a wallet policy with passkey authentication
   */
  async createUpdateWalletPolicyTransaction(
    params: types.UpdateWalletPolicyParams
  ): Promise<VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const signatureArgs = convertPasskeySignatureToInstructionArgs(
      params.passkeySignature
    );

    const updateInstruction = await this.buildUpdateWalletPolicyInstruction(
      params.payer,
      params.smartWallet,
      {
        ...signatureArgs,
        verifyInstructionIndex: 0,
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
        vaultIndex: params.vaultIndex || this.generateVaultIndex(),
      },
      params.destroyPolicyInstruction,
      params.initPolicyInstruction
    );

    const instructions = combineInstructionsWithAuth(authInstruction, [
      updateInstruction,
    ]);
    return buildVersionedTransaction(
      this.connection,
      params.payer,
      instructions
    );
  }

  /**
   * Creates a deferred execution with passkey authentication
   */
  async createDeferredExecutionTransaction(
    params: types.CreateDeferredExecutionParams
  ): Promise<VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const smartWalletId = await this.getSmartWalletData(
      params.smartWallet
    ).then((d) => d.walletId);

    let policyInstruction = await this.defaultPolicyProgram.buildCheckPolicyIx(
      smartWalletId,
      params.passkeySignature.passkeyPublicKey,
      this.walletDevicePda(
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

    const sessionInstruction =
      await this.buildCreateDeferredExecutionInstruction(
        params.payer,
        params.smartWallet,
        {
          ...signatureArgs,
          expiresAt: new BN(params.expiresAt),
          policyData: policyInstruction.data,
          verifyInstructionIndex: 0,
          vaultIndex: params.vaultIndex || this.generateVaultIndex(),
        },
        policyInstruction
      );

    const instructions = combineInstructionsWithAuth(authInstruction, [
      sessionInstruction,
    ]);
    return buildVersionedTransaction(
      this.connection,
      params.payer,
      instructions
    );
  }

  /**
   * Executes a deferred transaction (no authentication needed)
   */
  async createExecuteDeferredTransactionTransaction(
    params: types.ExecuteDeferredTransactionParams,
    vaultIndex?: number
  ): Promise<VersionedTransaction> {
    const vaultIdx = vaultIndex || this.generateVaultIndex();

    const instruction = await this.buildExecuteDeferredTransactionInstruction(
      params.payer,
      params.smartWallet,
      params.cpiInstructions,
      vaultIdx
    );

    return buildVersionedTransaction(this.connection, params.payer, [
      instruction,
    ]);
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
      case types.SmartWalletAction.ExecuteDirectTransaction: {
        const { policyInstruction: policyIns, cpiInstruction } =
          action.args as types.ArgsByAction[types.SmartWalletAction.ExecuteDirectTransaction];

        const smartWalletId = await this.getSmartWalletData(smartWallet).then(
          (d) => d.walletId
        );

        let policyInstruction =
          await this.defaultPolicyProgram.buildCheckPolicyIx(
            smartWalletId,
            passkeyPublicKey,
            this.walletDevicePda(smartWallet, passkeyPublicKey),
            params.smartWallet
          );

        if (policyIns) {
          policyInstruction = policyIns;
        }

        const smartWalletData = await this.getSmartWalletData(smartWallet);

        message = buildExecuteMessage(
          smartWallet,
          smartWalletData.lastNonce,
          new BN(Math.floor(Date.now() / 1000)),
          policyInstruction,
          cpiInstruction,
          [payer]
        );
        break;
      }
      case types.SmartWalletAction.InvokeWalletPolicy: {
        const { policyInstruction } =
          action.args as types.ArgsByAction[types.SmartWalletAction.InvokeWalletPolicy];

        const smartWalletData = await this.getSmartWalletData(smartWallet);

        message = buildInvokePolicyMessage(
          smartWallet,
          smartWalletData.lastNonce,
          new BN(Math.floor(Date.now() / 1000)),
          policyInstruction,
          [payer]
        );
        break;
      }
      case types.SmartWalletAction.UpdateWalletPolicy: {
        const { initPolicyIns, destroyPolicyIns } =
          action.args as types.ArgsByAction[types.SmartWalletAction.UpdateWalletPolicy];

        const smartWalletData = await this.getSmartWalletData(smartWallet);

        message = buildUpdatePolicyMessage(
          smartWallet,
          smartWalletData.lastNonce,
          new BN(Math.floor(Date.now() / 1000)),
          destroyPolicyIns,
          initPolicyIns
        );
        break;
      }

      default:
        throw new Error(`Unsupported SmartWalletAction: ${action.type}`);
    }

    return message;
  }
}
