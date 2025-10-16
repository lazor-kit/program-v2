import { Program, BN } from '@coral-xyz/anchor';
import {
  PublicKey,
  Transaction,
  TransactionInstruction,
  Connection,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  VersionedTransaction,
} from '@solana/web3.js';
import LazorkitIdl from '../anchor/idl/lazorkit.json';
import { Lazorkit } from '../anchor/types/lazorkit';
import {
  deriveConfigPda,
  derivePolicyProgramRegistryPda,
  deriveSmartWalletPda,
  deriveSmartWalletConfigPda,
  deriveChunkPda,
  deriveLazorkitVaultPda,
  deriveWalletDevicePda,
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
  getWalletStatePubkey(smartWallet: PublicKey): PublicKey {
    return deriveSmartWalletConfigPda(this.programId, smartWallet);
  }

  /**
   * Derives a wallet device PDA for a given smart wallet and passkey
   */
  getWalletDevicePubkey(
    smartWallet: PublicKey,
    credentialHash: number[]
  ): PublicKey {
    if (credentialHash.length !== 32) {
      throw new Error('Credential hash must be 32 bytes');
    }

    return deriveWalletDevicePda(
      this.programId,
      smartWallet,
      credentialHash
    )[0];
  }

  /**
   * Derives a transaction session PDA for a given smart wallet and nonce
   */
  getChunkPubkey(smartWallet: PublicKey, lastNonce: BN): PublicKey {
    return deriveChunkPda(this.programId, smartWallet, lastNonce);
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
    const smartWalletConfig = await this.getWalletStateData(smartWallet);
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
  async getWalletStateData(smartWallet: PublicKey) {
    const pda = this.getWalletStatePubkey(smartWallet);
    return await this.program.account.walletState.fetch(pda);
  }

  /**
   * Fetches transaction session data for a given transaction session
   */
  async getChunkData(chunk: PublicKey) {
    return await this.program.account.chunk.fetch(chunk);
  }

  /**
   * Finds a smart wallet by passkey public key
   * Searches through all WalletState accounts to find one containing the specified passkey
   */
  async getSmartWalletByPasskey(passkeyPublicKey: number[]): Promise<{
    smartWallet: PublicKey | null;
    walletState: PublicKey | null;
    deviceSlot: { passkeyPubkey: number[]; credentialHash: number[] } | null;
  }> {
    // Get the discriminator for WalletState accounts
    const discriminator = this.program.idl.accounts?.find(
      (a: any) => a.name === 'WalletState'
    )?.discriminator;

    if (!discriminator) {
      throw new Error('WalletState discriminator not found in IDL');
    }

    // Get all WalletState accounts
    const accounts = await this.connection.getProgramAccounts(this.programId, {
      filters: [{ memcmp: { offset: 0, bytes: bs58.encode(discriminator) } }],
    });

    // Search through each WalletState account
    for (const account of accounts) {
      try {
        // Deserialize the WalletState account data
        const walletStateData = this.program.coder.accounts.decode(
          'WalletState',
          account.account.data
        );

        // Check if any device contains the target passkey
        for (const device of walletStateData.devices) {
          if (this.arraysEqual(device.passkeyPubkey, passkeyPublicKey)) {
            // Found the matching device, return the smart wallet
            const smartWallet = this.getSmartWalletPubkey(
              walletStateData.walletId
            );
            return {
              smartWallet,
              walletState: account.pubkey,
              deviceSlot: {
                passkeyPubkey: device.passkeyPubkey,
                credentialHash: device.credentialHash,
              },
            };
          }
        }
      } catch (error) {
        // Skip accounts that can't be deserialized (might be corrupted or different type)
        continue;
      }
    }

    // No matching wallet found
    return {
      smartWallet: null,
      walletState: null,
      deviceSlot: null,
    };
  }

  /**
   * Helper method to compare two byte arrays
   */
  private arraysEqual(a: number[], b: number[]): boolean {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (a[i] !== b[i]) return false;
    }
    return true;
  }

  /**
   * Find smart wallet by credential hash
   * Searches through all WalletState accounts to find one containing the specified credential hash
   */
  async getSmartWalletByCredentialHash(credentialHash: number[]): Promise<{
    smartWallet: PublicKey | null;
    walletState: PublicKey | null;
    deviceSlot: { passkeyPubkey: number[]; credentialHash: number[] } | null;
  }> {
    // Get the discriminator for WalletState accounts
    const discriminator = this.program.idl.accounts?.find(
      (a: any) => a.name === 'WalletState'
    )?.discriminator;

    if (!discriminator) {
      throw new Error('WalletState discriminator not found in IDL');
    }

    // Get all WalletState accounts
    const accounts = await this.connection.getProgramAccounts(this.programId, {
      filters: [{ memcmp: { offset: 0, bytes: bs58.encode(discriminator) } }],
    });

    // Search through each WalletState account
    for (const account of accounts) {
      try {
        // Deserialize the WalletState account data
        const walletStateData = this.program.coder.accounts.decode(
          'WalletState',
          account.account.data
        );

        // Check if any device contains the target credential hash
        for (const device of walletStateData.devices) {
          if (this.arraysEqual(device.credentialHash, credentialHash)) {
            // Found the matching device, return the smart wallet
            const smartWallet = this.getSmartWalletPubkey(
              walletStateData.walletId
            );
            return {
              smartWallet,
              walletState: account.pubkey,
              deviceSlot: {
                passkeyPubkey: device.passkeyPubkey,
                credentialHash: device.credentialHash,
              },
            };
          }
        }
      } catch (error) {
        // Skip accounts that can't be deserialized (might be corrupted or different type)
        continue;
      }
    }

    // No matching wallet found
    return {
      smartWallet: null,
      walletState: null,
      deviceSlot: null,
    };
  }

  /**
   * Find smart wallet by either passkey public key or credential hash
   * This is a convenience method that tries both approaches
   */
  async findSmartWallet(
    passkeyPublicKey?: number[],
    credentialHash?: number[]
  ): Promise<{
    smartWallet: PublicKey | null;
    walletState: PublicKey | null;
    deviceSlot: { passkeyPubkey: number[]; credentialHash: number[] } | null;
    foundBy: 'passkey' | 'credential' | null;
  }> {
    // Try passkey first if provided
    if (passkeyPublicKey) {
      const result = await this.getSmartWalletByPasskey(passkeyPublicKey);
      if (result.smartWallet) {
        return { ...result, foundBy: 'passkey' as const };
      }
    }

    // Try credential hash if provided and passkey didn't work
    if (credentialHash) {
      const result = await this.getSmartWalletByCredentialHash(credentialHash);
      if (result.smartWallet) {
        return { ...result, foundBy: 'credential' as const };
      }
    }

    // No wallet found
    return {
      smartWallet: null,
      walletState: null,
      deviceSlot: null,
      foundBy: null,
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
    policyInstruction: TransactionInstruction,
    args: types.CreateSmartWalletArgs
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .createSmartWallet(args)
      .accountsPartial({
        payer,
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        smartWallet,
        walletState: this.getWalletStatePubkey(smartWallet),
        walletDevice: this.getWalletDevicePubkey(
          smartWallet,
          args.credentialHash
        ),
        lazorkitConfig: this.getConfigPubkey(),
        policyProgram: policyInstruction.programId,
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
    walletDevice: PublicKey,
    args: types.ExecuteArgs,
    policyInstruction: TransactionInstruction,
    cpiInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .execute(args)
      .accountsPartial({
        payer,
        smartWallet,
        walletState: this.getWalletStatePubkey(smartWallet),
        referral: await this.getReferralAccount(smartWallet),
        lazorkitVault: this.getLazorkitVaultPubkey(args.vaultIndex),
        walletDevice,
        policyProgramRegistry: this.getPolicyProgramRegistryPubkey(),
        policyProgram: policyInstruction.programId,
        cpiProgram: cpiInstruction.programId,
        lazorkitConfig: this.getConfigPubkey(),
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
    return await this.program.methods
      .callPolicy(args)
      .accountsPartial({
        payer,
        lazorkitConfig: this.getConfigPubkey(),
        smartWallet,
        walletState: this.getWalletStatePubkey(smartWallet),
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
      .remainingAccounts([...instructionToAccountMetas(policyInstruction)])
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
    return await this.program.methods
      .changePolicy(args)
      .accountsPartial({
        payer,
        lazorkitConfig: this.getConfigPubkey(),
        smartWallet,
        walletState: this.getWalletStatePubkey(smartWallet),
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
      .remainingAccounts([
        ...instructionToAccountMetas(destroyPolicyInstruction),
        ...instructionToAccountMetas(initPolicyInstruction),
      ])
      .instruction();
  }

  /**
   * Builds the create deferred execution instruction
   */
  async buildCreateChunkIns(
    payer: PublicKey,
    smartWallet: PublicKey,
    walletDevice: PublicKey,
    args: types.CreateChunkArgs,
    policyInstruction: TransactionInstruction
  ): Promise<TransactionInstruction> {
    return await this.program.methods
      .createChunk(args)
      .accountsPartial({
        payer,
        lazorkitConfig: this.getConfigPubkey(),
        smartWallet,
        walletState: this.getWalletStatePubkey(smartWallet),
        walletDevice,
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
        lazorkitConfig: this.getConfigPubkey(),
        smartWallet,
        walletState: this.getWalletStatePubkey(smartWallet),
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

    return await this.program.methods
      .closeChunk()
      .accountsPartial({
        payer,
        smartWallet,
        walletState: this.getWalletStatePubkey(smartWallet),
        chunk,
        sessionRefund,
      })
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
        lazorkitConfig: this.getConfigPubkey(),
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
    const walletState = this.getWalletStatePubkey(smartWallet);

    const credentialId = Buffer.from(params.credentialIdBase64, 'base64');
    const credentialHash = Array.from(
      new Uint8Array(require('js-sha256').arrayBuffer(credentialId))
    );

    const policySigner = this.getWalletDevicePubkey(
      smartWallet,
      credentialHash
    );

    let policyInstruction = await this.defaultPolicyProgram.buildInitPolicyIx(
      params.smartWalletId,
      params.passkeyPublicKey,
      credentialHash,
      policySigner,
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

    const walletStateData = await this.getWalletStateData(params.smartWallet);

    const smartWalletId = walletStateData.walletId;

    const policySigner = this.getWalletDevicePubkey(
      params.smartWallet,
      params.credentialHash
    );

    let policyInstruction = await this.defaultPolicyProgram.buildCheckPolicyIx(
      smartWalletId,
      params.passkeySignature.passkeyPublicKey,
      policySigner,
      params.smartWallet,
      params.credentialHash,
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
      policySigner,
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
              credentialHash: Array.from(
                require('js-sha256').arrayBuffer(
                  Buffer.from(
                    params.newWalletDevice.credentialIdBase64,
                    'base64'
                  )
                )
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
              credentialHash: Array.from(
                require('js-sha256').arrayBuffer(
                  Buffer.from(
                    params.newWalletDevice.credentialIdBase64,
                    'base64'
                  )
                )
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

    const walletStateData = await this.getWalletStateData(params.smartWallet);

    const walletDevice = this.getWalletDevicePubkey(
      params.smartWallet,
      params.credentialHash
    );

    let policyInstruction = await this.defaultPolicyProgram.buildCheckPolicyIx(
      walletStateData.walletId,
      params.passkeySignature.passkeyPublicKey,
      walletDevice,
      params.smartWallet,
      params.credentialHash,
      walletStateData.policyData
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
      walletDevice,
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
    credentialHash: number[];
  }): Promise<Buffer> {
    let message: Buffer;
    const { action, payer, smartWallet, passkeyPublicKey } = params;

    switch (action.type) {
      case types.SmartWalletAction.Execute: {
        const { policyInstruction: policyIns, cpiInstruction } =
          action.args as types.ArgsByAction[types.SmartWalletAction.Execute];

        const walletStateData = await this.getWalletStateData(
          params.smartWallet
        );

        const policySigner = this.getWalletDevicePubkey(
          params.smartWallet,
          params.credentialHash
        );

        let policyInstruction =
          await this.defaultPolicyProgram.buildCheckPolicyIx(
            walletStateData.walletId,
            passkeyPublicKey,
            policySigner,
            params.smartWallet,
            params.credentialHash,
            walletStateData.policyData
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
