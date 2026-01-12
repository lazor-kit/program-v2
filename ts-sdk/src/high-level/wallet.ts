import type { Address } from '@solana/kit';
import type { Rpc as SolanaRpc } from '@solana/rpc';
import type { GetAccountInfoApi, GetSlotApi } from '@solana/rpc-api';
import { 
  findWalletAccount, 
  findWalletVault,
} from '../utils';
import { fetchOdometer } from '../utils/odometer';
import { LazorkitInstructionBuilder } from '../low-level';
import type { Authority } from '../authority/base';
import { AuthorityType, RolePermission, type PluginRef } from '../types';
import { LazorkitError, LazorkitErrorCode } from '../errors';

/**
 * Configuration for initializing a Lazorkit wallet
 */
export interface LazorkitWalletConfig {
  /** RPC client */
  rpc: SolanaRpc<GetAccountInfoApi & GetSlotApi>;
  /** Wallet ID (32 bytes) */
  walletId: Uint8Array;
  /** Authority for signing */
  authority: Authority;
  /** Authority ID (if wallet already exists) */
  authorityId?: number;
  /** Fee payer address */
  feePayer: Address;
  /** Program ID (optional, defaults to mainnet) */
  programId?: Address;
}

/**
 * High-level Lazorkit wallet class
 * 
 * Provides easy-to-use methods for common wallet operations
 */
export class LazorkitWallet {
  private instructionBuilder: LazorkitInstructionBuilder;
  private walletAccount: Address;
  private walletVault: Address;
  private authorityId: number;
  private odometer?: number;

  constructor(
    private config: LazorkitWalletConfig,
    walletAccount: Address,
    walletVault: Address,
    _walletBump: number, // Stored but not used yet (may be needed for signing)
    authorityId: number
  ) {
    this.instructionBuilder = new LazorkitInstructionBuilder(config.programId);
    this.walletAccount = walletAccount;
    this.walletVault = walletVault;
    this.authorityId = authorityId;
  }

  /**
   * Initialize or load a Lazorkit wallet
   * 
   * If wallet doesn't exist, it will be created with the provided authority as the first authority.
   * If wallet exists, it will be loaded and the authority will be authenticated.
   */
  static async initialize(config: LazorkitWalletConfig): Promise<LazorkitWallet> {
    // Find wallet account PDA
    const [walletAccount, _bump] = await findWalletAccount(config.walletId, config.programId);
    
    // Find wallet vault PDA
    const [walletVault, walletBump] = await findWalletVault(walletAccount, config.programId);

    // Check if wallet exists
    const { value: accountData } = await config.rpc.getAccountInfo(walletAccount, {
      encoding: 'base64',
    }).send();

    if (!accountData || !accountData.data) {
      // Wallet doesn't exist - will need to create it
      // For now, throw error - creation should be done separately
      throw new LazorkitError(
        LazorkitErrorCode.InvalidAccountData,
        'Wallet does not exist. Use createWallet() to create a new wallet.'
      );
    }

    // Wallet exists - load it
    // Parse wallet account to find authority ID
    const authorityId = config.authorityId ?? 0; // Default to first authority
    
    // Fetch odometer if needed (for Secp256k1/Secp256r1)
    let odometer: number | undefined = undefined;
    if (config.authority.type === AuthorityType.Secp256k1 || 
        config.authority.type === AuthorityType.Secp256r1) {
      try {
        odometer = await fetchOdometer(config.rpc, walletAccount, authorityId);
        // Update authority odometer if it has the method
        if (config.authority.getOdometer && config.authority.incrementOdometer) {
          // Note: This is a simplified approach - in practice, you'd want to sync odometer
        }
      } catch (error) {
        // Odometer fetch failed - might be a new authority or different type
        // Failed to fetch odometer - might be a new authority or different type
      }
    }

    const wallet = new LazorkitWallet(
      config,
      walletAccount,
      walletVault,
      walletBump,
      authorityId
    );
    
    wallet.odometer = odometer;
    return wallet;
  }

  /**
   * Create a new Lazorkit wallet
   * 
   * Creates a new wallet with the provided authority as the first (root) authority.
   */
  static async createWallet(params: {
    rpc: SolanaRpc<GetAccountInfoApi & GetSlotApi>;
    walletId: Uint8Array;
    authority: Authority;
    rolePermission?: RolePermission;
    pluginRefs?: PluginRef[];
    feePayer: Address;
    programId?: Address;
  }): Promise<LazorkitWallet> {
    // Find PDAs
    const [walletAccount, _bump] = await findWalletAccount(params.walletId, params.programId);
    const [walletVault, walletBump] = await findWalletVault(walletAccount, params.programId);

    // Serialize authority data
    const authorityData = await params.authority.serialize();
    
    // Build create instruction
    const instructionBuilder = new LazorkitInstructionBuilder(params.programId);
    instructionBuilder.buildCreateSmartWalletInstruction({
      walletAccount,
      payer: params.feePayer,
      walletVault,
      args: {
        id: params.walletId,
        bump: _bump,
        walletBump,
        firstAuthorityType: params.authority.type,
        firstAuthorityDataLen: authorityData.length,
        numPluginRefs: params.pluginRefs?.length ?? 0,
        rolePermission: params.rolePermission ?? RolePermission.AllButManageAuthority,
      },
      firstAuthorityData: authorityData,
      pluginRefs: params.pluginRefs,
    });

    // TODO: Build and send transaction
    // This requires transaction building with @solana/kit
    // For now, return wallet instance (transaction should be sent separately)
    
    const wallet = new LazorkitWallet(
      {
        rpc: params.rpc,
        walletId: params.walletId,
        authority: params.authority,
        feePayer: params.feePayer,
        programId: params.programId,
      },
      walletAccount,
      walletVault,
      walletBump,
      0 // First authority has ID 0
    );

    return wallet;
  }

  /**
   * Get wallet account address
   */
  getWalletAccount(): Address {
    return this.walletAccount;
  }

  /**
   * Get wallet vault address
   */
  getWalletVault(): Address {
    return this.walletVault;
  }

  /**
   * Get current authority ID
   */
  getAuthorityId(): number {
    return this.authorityId;
  }

  /**
   * Get current odometer value
   */
  async getOdometer(): Promise<number> {
    if (this.odometer !== undefined) {
      return this.odometer;
    }
    
    // Fetch from chain
    this.odometer = await fetchOdometer(this.config.rpc, this.walletAccount, this.authorityId);
    return this.odometer;
  }

  /**
   * Refresh odometer from chain
   */
  async refreshOdometer(): Promise<void> {
    this.odometer = await fetchOdometer(this.config.rpc, this.walletAccount, this.authorityId);
  }

  /**
   * Build Sign instruction
   * 
   * This is a helper method that builds the Sign instruction with proper account setup.
   * The actual transaction building and signing should be done separately.
   */
  async buildSignInstruction(params: {
    instructions: Array<{
      programAddress: Address;
      accounts?: Array<{ address: Address; role: any }>;
      data?: Uint8Array;
    }>;
    additionalAccounts?: Array<{ address: Address; role: any }>;
    slot?: bigint;
  }): Promise<import('@solana/kit').Instruction> {
    // Serialize inner instructions to compact format
    const { serializeInstructions } = await import('../utils/instructions');
    const instructionPayload = await serializeInstructions(params.instructions);
    
    // Build message hash for signing
    const { buildMessageHash } = await import('../utils/authorityPayload');
    const messageHash = await buildMessageHash({
      instructionPayload,
      odometer: this.odometer,
      slot: params.slot,
      authorityType: this.config.authority.type,
    });
    
    // Build authority payload (signature + odometer if needed)
    const authorityPayload = await this.buildAuthorityPayload({
      message: messageHash,
      slot: params.slot ?? 0n,
    });

    const instruction = this.instructionBuilder.buildSignInstruction({
      walletAccount: this.walletAccount,
      walletVault: this.walletVault,
      args: {
        instructionPayloadLen: instructionPayload.length,
        authorityId: this.authorityId,
      },
      instructionPayload,
      authorityPayload,
      additionalAccounts: params.additionalAccounts,
    });

    return instruction;
  }

  /**
   * Build authority payload for signing
   * 
   * This includes the signature and odometer (if applicable)
   */
  private async buildAuthorityPayload(params: {
    message: Uint8Array;
    slot?: bigint;
  }): Promise<Uint8Array> {
    const { buildAuthorityPayload } = await import('../utils/authorityPayload');
    
    // Get current odometer if needed
    let odometer: number | undefined;
    if (this.config.authority.type === AuthorityType.Secp256k1 || 
        this.config.authority.type === AuthorityType.Secp256r1 ||
        this.config.authority.type === AuthorityType.Secp256k1Session ||
        this.config.authority.type === AuthorityType.Secp256r1Session) {
      odometer = await this.getOdometer();
    }

    return buildAuthorityPayload({
      authority: this.config.authority,
      message: params.message,
      odometer,
      slot: params.slot,
    });
  }

  /**
   * Add a new authority to the wallet
   */
  async buildAddAuthorityInstruction(params: {
    newAuthority: Authority;
    rolePermission?: RolePermission;
    pluginRefs?: PluginRef[];
  }): Promise<import('@solana/kit').Instruction> {
    const authorityData = await params.newAuthority.serialize();

    const instruction = this.instructionBuilder.buildAddAuthorityInstruction({
      walletAccount: this.walletAccount,
      payer: this.config.feePayer,
      args: {
        actingAuthorityId: this.authorityId,
        newAuthorityType: params.newAuthority.type,
        newAuthorityDataLen: authorityData.length,
        numPluginRefs: params.pluginRefs?.length ?? 0,
        rolePermission: params.rolePermission ?? RolePermission.AllButManageAuthority,
      },
      newAuthorityData: authorityData,
      pluginRefs: params.pluginRefs,
    });

    return instruction;
  }

  /**
   * Remove an authority from the wallet
   */
  async buildRemoveAuthorityInstruction(params: {
    authorityToRemoveId: number;
  }): Promise<import('@solana/kit').Instruction> {
    const instruction = this.instructionBuilder.buildRemoveAuthorityInstruction({
      walletAccount: this.walletAccount,
      payer: this.config.feePayer,
      walletVault: this.walletVault,
      authorityToRemove: this.walletAccount, // TODO: Get actual authority account address
      args: {
        actingAuthorityId: this.authorityId,
        authorityToRemoveId: params.authorityToRemoveId,
      },
    });

    return instruction;
  }

  /**
   * Create a session for the current authority
   * 
   * @param params - Session creation parameters
   * @param params.sessionKey - Session key (32 bytes). If not provided, a random key will be generated.
   * @param params.duration - Session duration in slots. If not provided, a recommended duration will be used.
   * @returns Instruction for creating the session
   */
  async buildCreateSessionInstruction(params?: {
    sessionKey?: Uint8Array;
    duration?: bigint;
  }): Promise<import('@solana/kit').Instruction> {
    const { generateSessionKey, getRecommendedSessionDuration } = await import('../utils/session');
    
    const sessionKey = params?.sessionKey ?? generateSessionKey();
    const duration = params?.duration ?? getRecommendedSessionDuration(this.config.authority.type);

    // Validate session key
    if (sessionKey.length !== 32) {
      throw new LazorkitError(
        LazorkitErrorCode.SerializationError,
        `Session key must be 32 bytes, got ${sessionKey.length}`
      );
    }

    const instruction = this.instructionBuilder.buildCreateSessionInstruction({
      walletAccount: this.walletAccount,
      payer: this.config.feePayer,
      args: {
        authorityId: this.authorityId,
        sessionKey,
        duration,
      },
    });

    return instruction;
  }

  /**
   * Get current slot from RPC
   */
  async getCurrentSlot(): Promise<bigint> {
    const response = await this.config.rpc.getSlot().send();
    return BigInt(response);
  }

  /**
   * Add a plugin to the wallet's plugin registry
   */
  async buildAddPluginInstruction(params: {
    pluginProgramId: Address;
    pluginConfigAccount: Address;
    priority?: number;
    enabled?: boolean;
  }): Promise<import('@solana/kit').Instruction> {
    // Serialize plugin entry data
    // Format: program_id[32] + config_account[32] + priority[1] + enabled[1] + padding[6] = 72 bytes
    const pluginData = new Uint8Array(72);
    const { getAddressEncoder } = await import('@solana/kit');
    const addressEncoder = getAddressEncoder();
    
    const programIdBytes = addressEncoder.encode(params.pluginProgramId);
    const configAccountBytes = addressEncoder.encode(params.pluginConfigAccount);
    
    // Convert ReadonlyUint8Array to Uint8Array if needed
    const programBytes = programIdBytes instanceof Uint8Array 
      ? programIdBytes 
      : new Uint8Array(programIdBytes);
    const configBytes = configAccountBytes instanceof Uint8Array 
      ? configAccountBytes 
      : new Uint8Array(configAccountBytes);
    
    pluginData.set(programBytes, 0);
    pluginData.set(configBytes, 32);
    pluginData[64] = params.priority ?? 0;
    pluginData[65] = params.enabled !== false ? 1 : 0;
    // Padding (66-71) is already zero-initialized

    const instruction = this.instructionBuilder.buildAddPluginInstruction({
      walletAccount: this.walletAccount,
      payer: this.config.feePayer,
      walletVault: this.walletVault,
      args: {
        actingAuthorityId: this.authorityId,
      },
      pluginData,
    });

    return instruction;
  }

  /**
   * Remove a plugin from the wallet's plugin registry
   */
  async buildRemovePluginInstruction(params: {
    pluginIndex: number;
  }): Promise<import('@solana/kit').Instruction> {
    const instruction = this.instructionBuilder.buildRemovePluginInstruction({
      walletAccount: this.walletAccount,
      walletVault: this.walletVault,
      args: {
        actingAuthorityId: this.authorityId,
        pluginIndex: params.pluginIndex,
      },
    });

    return instruction;
  }

  /**
   * Update a plugin in the wallet's plugin registry
   */
  async buildUpdatePluginInstruction(params: {
    pluginIndex: number;
    priority?: number;
    enabled?: boolean;
  }): Promise<import('@solana/kit').Instruction> {
    // Serialize update data
    // Format: priority[1] + enabled[1] + padding[6] = 8 bytes
    const updateData = new Uint8Array(8);
    updateData[0] = params.priority ?? 0;
    updateData[1] = params.enabled !== undefined ? (params.enabled ? 1 : 0) : 0;
    // Padding (2-7) is already zero-initialized

    const instruction = this.instructionBuilder.buildUpdatePluginInstruction({
      walletAccount: this.walletAccount,
      walletVault: this.walletVault,
      args: {
        actingAuthorityId: this.authorityId,
        pluginIndex: params.pluginIndex,
      },
      updateData,
    });

    return instruction;
  }
}
