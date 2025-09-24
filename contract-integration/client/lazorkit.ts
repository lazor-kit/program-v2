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
  getSmartWalletConfigDataPubkey(smartWallet: PublicKey): PublicKey {
    return deriveSmartWalletConfigPda(this.programId, smartWallet);
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
  private async getReferralAccount(smartWallet: PublicKey): Promise<PublicKey> {
    const smartWalletConfig = await this.getSmartWalletConfigData(smartWallet);
    return smartWalletConfig.referralAddress;
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
  async getSmartWalletConfigData(smartWallet: PublicKey) {
    const pda = this.getSmartWalletConfigDataPubkey(smartWallet);
    return await this.program.account.smartWalletConfig.fetch(pda);
  }

  /**
   * Fetches wallet device data for a given device
   */
  async getWalletDeviceData(walletDevice: PublicKey) {
    return await this.program.account.walletDevice.fetch(walletDevice);
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
    walletDevice: PublicKey,
    policyInstruction: TransactionInstruction,
    args: types.CreateSmartWalletArgs
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .createSmartWallet(args)
      .accountsPartial({
        payer,
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        smartWallet,
        smartWalletConfig: this.getSmartWalletConfigDataPubkey(smartWallet),
        walletDevice,
        config: this.getConfigPubkey(),
        defaultPolicyProgram: this.defaultPolicyProgram.programId,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts([
        ...instructionToAccountMetas(policyInstruction, payer),
      ])
      .instruction();
  }

  /**
   * Builds the execute direct transaction instruction
   */
  async buildExecuteIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    args: types.ExecuteArgs,
    policyInstruction: TransactionInstruction,
    cpiInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .execute(args)
      .accountsPartial({
        payer,
        smartWallet,
        smartWalletConfig: this.getSmartWalletConfigDataPubkey(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.getLazorkitVaultPubkey(args.vaultIndex),
        walletDevice: this.getWalletDevicePubkey(
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
        ...instructionToAccountMetas(cpiInstruction, payer),
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
        smartWalletConfig: this.getSmartWalletConfigDataPubkey(smartWallet),
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
        smartWalletConfig: this.getSmartWalletConfigDataPubkey(smartWallet),
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
        smartWalletConfig: this.getSmartWalletConfigDataPubkey(smartWallet),
        walletDevice: this.getWalletDevicePubkey(
          smartWallet,
          args.passkeyPublicKey
        ),
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        policyProgram: policyInstruction.programId,
        chunk: this.getChunkPubkey(
          smartWallet,
          await this.getSmartWalletConfigData(smartWallet).then(
            (d) => d.lastNonce
          )
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
    const cfg = await this.getSmartWalletConfigData(smartWallet);
    const chunk = this.getChunkPubkey(smartWallet, cfg.lastNonce);

    const vaultIndex = await this.getChunkData(chunk).then((d) => d.vaultIndex);

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
      ...instructionToAccountMetas(ix, payer),
    ]);

    return await this.program.methods
      .executeChunk(instructionDataList, Buffer.from(splitIndex))
      .accountsPartial({
        payer,
        config: this.getConfigPubkey(),
        smartWallet,
        smartWalletConfig: this.getSmartWalletConfigDataPubkey(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.getLazorkitVaultPubkey(vaultIndex), // Will be updated based on session
        chunk,
        sessionRefund: payer,
        systemProgram: SystemProgram.programId,
      })
      .remainingAccounts(allAccountMetas)
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
      instructionToAccountMetas(ix, payer)
    );

    return await this.program.methods
      .grantPermission(args)
      .accountsPartial({
        payer,
        config: this.getConfigPubkey(),
        smartWallet,
        smartWalletConfig: this.getSmartWalletConfigDataPubkey(smartWallet),
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
      instructionToAccountMetas(ix, feePayer)
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
        smartWalletConfig: this.getSmartWalletConfigDataPubkey(smartWallet),
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
        config: this.getConfigPubkey(),
        vault: this.getLazorkitVaultPubkey(params.vaultIndex),
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
  async createSmartWalletTxn(params: types.CreateSmartWalletParams): Promise<{
    transaction: Transaction;
    smartWalletId: BN;
    smartWallet: PublicKey;
  }> {
    const smartWalletId = params.smartWalletId || this.generateWalletId();
    const smartWallet = this.getSmartWalletPubkey(smartWalletId);
    const walletDevice = this.getWalletDevicePubkey(
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
      vaultIndex: getVaultIndex(params.vaultIndex, () =>
        this.generateVaultIndex()
      ),
    };

    const instruction = await this.buildCreateSmartWalletIns(
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
  async executeTxn(params: types.ExecuteParams): Promise<VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const smartWalletId = await this.getSmartWalletConfigData(
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

    const execInstruction = await this.buildExecuteIns(
      params.payer,
      params.smartWallet,
      {
        ...signatureArgs,
        verifyInstructionIndex: 0,
        splitIndex: policyInstruction.keys.length,
        policyData: policyInstruction.data,
        cpiData: params.cpiInstruction.data,
        timestamp: new BN(Math.floor(Date.now() / 1000)),
        vaultIndex:
          params.vaultIndex !== undefined
            ? params.vaultIndex
            : this.generateVaultIndex(),
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
  async callPolicyTxn(
    params: types.CallPolicyParams
  ): Promise<VersionedTransaction> {
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
        verifyInstructionIndex: 0,
        timestamp: new BN(Math.floor(Date.now() / 1000)),
        vaultIndex: getVaultIndex(params.vaultIndex, () =>
          this.generateVaultIndex()
        ),
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
  async changePolicyTxn(
    params: types.ChangePolicyParams
  ): Promise<VersionedTransaction> {
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
    return buildVersionedTransaction(
      this.connection,
      params.payer,
      instructions
    );
  }

  /**
   * Creates a deferred execution with passkey authentication
   */
  async createChunkTxn(
    params: types.CreateChunkParams
  ): Promise<VersionedTransaction> {
    const authInstruction = buildPasskeyVerificationInstruction(
      params.passkeySignature
    );

    const smartWalletId = await this.getSmartWalletConfigData(
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
        policyData: policyInstruction.data,
        verifyInstructionIndex: 0,
        timestamp: new BN(Math.floor(Date.now() / 1000)),
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
    return buildVersionedTransaction(
      this.connection,
      params.payer,
      instructions
    );
  }

  /**
   * Executes a deferred transaction (no authentication needed)
   */
  async executeChunkTxn(
    params: types.ExecuteChunkParams
  ): Promise<VersionedTransaction> {
    const instruction = await this.buildExecuteChunkIns(
      params.payer,
      params.smartWallet,
      params.cpiInstructions
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
      case types.SmartWalletAction.Execute: {
        const { policyInstruction: policyIns, cpiInstruction } =
          action.args as types.ArgsByAction[types.SmartWalletAction.Execute];

        const smartWalletId = await this.getSmartWalletConfigData(
          smartWallet
        ).then((d) => d.walletId);

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

        const smartWalletConfig = await this.getSmartWalletConfigData(
          smartWallet
        );

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

        const smartWalletConfig = await this.getSmartWalletConfigData(
          smartWallet
        );

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

        const smartWalletConfig = await this.getSmartWalletConfigData(
          smartWallet
        );

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

        const smartWalletConfig = await this.getSmartWalletConfigData(
          smartWallet
        );

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
