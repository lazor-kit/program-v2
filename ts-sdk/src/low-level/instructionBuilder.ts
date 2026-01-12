import type { Address, Instruction } from '@solana/kit';
import { AccountRole, type AccountMeta } from '@solana/instructions';
import { LAZORKIT_PROGRAM_ID } from '../utils/pda';
import { LazorkitError, LazorkitErrorCode } from '../errors';
import { LazorkitInstruction } from '../instructions/types';
import {
  serializeCreateSmartWalletArgs,
  serializeSignArgs,
  serializeAddAuthorityArgs,
  serializeUpdateAuthorityArgs,
  serializeRemoveAuthorityArgs,
  serializeAddPluginArgs,
  serializeRemovePluginArgs,
  serializeUpdatePluginArgs,
  serializeCreateSessionArgs,
  serializePluginRefs,
  writeInstructionDiscriminator,
} from '../utils/serialization';
import type {
  CreateSmartWalletArgs,
  SignArgs,
  AddAuthorityArgs,
  UpdateAuthorityArgs,
  RemoveAuthorityArgs,
  AddPluginArgs,
  RemovePluginArgs,
  UpdatePluginArgs,
  CreateSessionArgs,
} from '../instructions/types';

/**
 * System Program ID
 */
const SYSTEM_PROGRAM_ID: Address = '11111111111111111111111111111111' as Address;

/**
 * Low-level instruction builder for Lazorkit V2
 * 
 * Provides full control over instruction building for pro developers
 */
export class LazorkitInstructionBuilder {
  constructor(
    private programId: Address = LAZORKIT_PROGRAM_ID
  ) { }

  /**
   * Build CreateSmartWallet instruction
   * 
   * Accounts:
   * 0. wallet_account (writable, PDA)
   * 1. wallet_vault (writable, PDA)
   * 2. payer (writable, signer)
   * 3. system_program
   */
  buildCreateSmartWalletInstruction(params: {
    walletAccount: Address;
    payer: Address;
    walletVault: Address;
    args: CreateSmartWalletArgs;
    firstAuthorityData: Uint8Array;
    pluginRefs?: import('../types').PluginRef[];
  }): Instruction {
    // Serialize instruction data
    const argsData = serializeCreateSmartWalletArgs(params.args);

    // Combine: discriminator (2 bytes) + args (43 bytes) + first_authority_data + plugin_refs
    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.CreateSmartWallet);

    // Serialize plugin refs if provided
    const pluginRefsData = params.pluginRefs && params.pluginRefs.length > 0
      ? serializePluginRefs(params.pluginRefs)
      : new Uint8Array(0);

    const totalDataLength = 2 + 48 + params.firstAuthorityData.length + pluginRefsData.length;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);
    offset += 48;
    instructionData.set(params.firstAuthorityData, offset);
    offset += params.firstAuthorityData.length;
    if (pluginRefsData.length > 0) {
      instructionData.set(pluginRefsData, offset);
    }

    return {
      programAddress: this.programId,
      accounts: [
        { address: params.walletAccount, role: AccountRole.WRITABLE },
        { address: params.walletVault, role: AccountRole.WRITABLE },
        { address: params.payer, role: AccountRole.WRITABLE_SIGNER },
        { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
      ],
      data: instructionData,
    };
  }

  /**
   * Build Sign instruction
   * 
   * Accounts:
   * 0. wallet_account (writable)
   * 1. wallet_vault (writable, signer, PDA)
   * 2..N. Accounts for inner instructions
   */
  buildSignInstruction(params: {
    walletAccount: Address;
    walletVault: Address;
    args: SignArgs;
    instructionPayload: Uint8Array;
    authorityPayload: Uint8Array;
    additionalAccounts?: AccountMeta[];
  }): Instruction {
    // Serialize instruction data
    const argsData = serializeSignArgs(params.args);

    // Combine: discriminator (2 bytes) + args (6 bytes) + instruction_payload + authority_payload
    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.Sign);

    const totalDataLength = 2 + 8 + params.instructionPayload.length + params.authorityPayload.length;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);
    offset += 8;
    instructionData.set(params.instructionPayload, offset);
    offset += params.instructionPayload.length;
    instructionData.set(params.authorityPayload, offset);

    const accounts: AccountMeta[] = [
      { address: params.walletAccount, role: AccountRole.WRITABLE },
      { address: params.walletVault, role: AccountRole.WRITABLE_SIGNER },
    ];

    if (params.additionalAccounts) {
      accounts.push(...params.additionalAccounts);
    }

    return {
      programAddress: this.programId,
      accounts,
      data: instructionData,
    };
  }

  /**
   * Build AddAuthority instruction
   * 
   * Accounts:
   * 0. wallet_account (writable)
   * 1. payer (writable, signer)
   * 2. system_program
   */
  buildAddAuthorityInstruction(params: {
    walletAccount: Address;
    payer: Address;
    args: AddAuthorityArgs;
    newAuthorityData: Uint8Array;
    pluginRefs?: import('../types').PluginRef[];
  }): Instruction {
    // Validate data length matches declared length
    if (params.newAuthorityData.length !== params.args.newAuthorityDataLen) {
      throw new LazorkitError(
        LazorkitErrorCode.SerializationError,
        `New authority data length mismatch: declared ${params.args.newAuthorityDataLen}, actual ${params.newAuthorityData.length}`
      );
    }

    // Validate plugin refs count
    const actualPluginRefsCount = params.pluginRefs?.length || 0;
    if (actualPluginRefsCount !== params.args.numPluginRefs) {
      throw new LazorkitError(
        LazorkitErrorCode.SerializationError,
        `Plugin refs count mismatch: declared ${params.args.numPluginRefs}, actual ${actualPluginRefsCount}`
      );
    }
    const argsData = serializeAddAuthorityArgs(params.args);

    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.AddAuthority);

    // Serialize plugin refs if provided
    const pluginRefsData = params.pluginRefs && params.pluginRefs.length > 0
      ? serializePluginRefs(params.pluginRefs)
      : new Uint8Array(0);

    const totalDataLength = 2 + 16 + params.newAuthorityData.length + pluginRefsData.length;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);
    offset += 16;
    instructionData.set(params.newAuthorityData, offset);
    offset += params.newAuthorityData.length;
    if (pluginRefsData.length > 0) {
      instructionData.set(pluginRefsData, offset);
    }

    return {
      programAddress: this.programId,
      accounts: [
        { address: params.walletAccount, role: AccountRole.WRITABLE },
        { address: params.payer, role: AccountRole.WRITABLE_SIGNER },
        { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
      ],
      data: instructionData,
    };
  }

  /**
   * Build UpdateAuthority instruction
   * 
   * Accounts:
   * 0. wallet_account (writable)
   * 1. wallet_vault (signer, PDA)
   * 2. authority_to_update (writable)
   */
  buildUpdateAuthorityInstruction(params: {
    walletAccount: Address;
    walletVault: Address;
    authorityToUpdate: Address;
    args: UpdateAuthorityArgs;
    updateData?: Uint8Array;
  }): Instruction {
    const argsData = serializeUpdateAuthorityArgs(params.args);

    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.UpdateAuthority);

    const updateData = params.updateData || new Uint8Array(0);
    const totalDataLength = 2 + 8 + updateData.length;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);
    offset += 8;
    if (updateData.length > 0) {
      instructionData.set(updateData, offset);
    }

    return {
      programAddress: this.programId,
      accounts: [
        { address: params.walletAccount, role: AccountRole.WRITABLE },
        { address: params.walletVault, role: AccountRole.READONLY_SIGNER },
        { address: params.authorityToUpdate, role: AccountRole.WRITABLE },
      ],
      data: instructionData,
    };
  }

  /**
   * Build RemoveAuthority instruction
   * 
   * Accounts:
   * 0. wallet_account (writable)
   * 1. payer (writable, signer)
   * 2. wallet_vault (signer, PDA)
   * 3. authority_to_remove (writable)
   */
  buildRemoveAuthorityInstruction(params: {
    walletAccount: Address;
    payer: Address;
    walletVault: Address;
    authorityToRemove: Address;
    args: RemoveAuthorityArgs;
  }): Instruction {
    const argsData = serializeRemoveAuthorityArgs(params.args);

    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.RemoveAuthority);

    const totalDataLength = 2 + 8;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);

    return {
      programAddress: this.programId,
      accounts: [
        { address: params.walletAccount, role: AccountRole.WRITABLE },
        { address: params.payer, role: AccountRole.WRITABLE_SIGNER },
        { address: params.walletVault, role: AccountRole.READONLY_SIGNER },
        { address: params.authorityToRemove, role: AccountRole.WRITABLE },
      ],
      data: instructionData,
    };
  }

  /**
   * Build AddPlugin instruction
   * 
   * Accounts:
   * 0. wallet_account (writable)
   * 1. payer (writable, signer)
   * 2. wallet_vault (signer, PDA)
   */
  buildAddPluginInstruction(params: {
    walletAccount: Address;
    payer: Address;
    walletVault: Address;
    args: AddPluginArgs;
    pluginData: Uint8Array;
  }): Instruction {
    const argsData = serializeAddPluginArgs(params.args);

    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.AddPlugin);

    const totalDataLength = 2 + 8 + params.pluginData.length;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);
    offset += 8;
    instructionData.set(params.pluginData, offset);

    return {
      programAddress: this.programId,
      accounts: [
        { address: params.walletAccount, role: AccountRole.WRITABLE },
        { address: params.payer, role: AccountRole.WRITABLE_SIGNER },
        { address: params.walletVault, role: AccountRole.READONLY_SIGNER },
      ],
      data: instructionData,
    };
  }

  /**
   * Build RemovePlugin instruction
   * 
   * Accounts:
   * 0. wallet_account (writable)
   * 1. wallet_vault (signer, PDA)
   */
  buildRemovePluginInstruction(params: {
    walletAccount: Address;
    walletVault: Address;
    args: RemovePluginArgs;
  }): Instruction {
    const argsData = serializeRemovePluginArgs(params.args);

    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.RemovePlugin);

    const totalDataLength = 2 + 8;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);

    return {
      programAddress: this.programId,
      accounts: [
        { address: params.walletAccount, role: AccountRole.WRITABLE },
        { address: params.walletVault, role: AccountRole.READONLY_SIGNER },
      ],
      data: instructionData,
    };
  }

  /**
   * Build UpdatePlugin instruction
   * 
   * Accounts:
   * 0. wallet_account (writable)
   * 1. wallet_vault (signer, PDA)
   */
  buildUpdatePluginInstruction(params: {
    walletAccount: Address;
    walletVault: Address;
    args: UpdatePluginArgs;
    updateData?: Uint8Array;
  }): Instruction {
    const argsData = serializeUpdatePluginArgs(params.args);

    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.UpdatePlugin);

    const updateData = params.updateData || new Uint8Array(0);
    const totalDataLength = 2 + 8 + updateData.length;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);
    offset += 8;
    if (updateData.length > 0) {
      instructionData.set(updateData, offset);
    }

    return {
      programAddress: this.programId,
      accounts: [
        { address: params.walletAccount, role: AccountRole.WRITABLE },
        { address: params.walletVault, role: AccountRole.READONLY_SIGNER },
      ],
      data: instructionData,
    };
  }

  /**
   * Build CreateSession instruction
   * 
   * Accounts:
   * 0. wallet_account (writable)
   * 1. payer (writable, signer)
   */
  buildCreateSessionInstruction(params: {
    walletAccount: Address;
    payer: Address;
    args: CreateSessionArgs;
  }): Instruction {
    const argsData = serializeCreateSessionArgs(params.args);

    const discriminatorBuffer = new Uint8Array(2);
    writeInstructionDiscriminator(discriminatorBuffer, LazorkitInstruction.CreateSession);

    const totalDataLength = 2 + 48;
    const instructionData = new Uint8Array(totalDataLength);

    let offset = 0;
    instructionData.set(discriminatorBuffer, offset);
    offset += 2;
    instructionData.set(argsData, offset);

    return {
      programAddress: this.programId,
      accounts: [
        { address: params.walletAccount, role: AccountRole.WRITABLE },
        { address: params.payer, role: AccountRole.WRITABLE_SIGNER },
      ],
      data: instructionData,
    };
  }
}
