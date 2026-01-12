import { LazorkitError, LazorkitErrorCode } from '../errors';
import type { PluginRef } from '../types';

/**
 * Write u8 to buffer at offset
 */
function writeU8(buffer: Uint8Array, offset: number, value: number): void {
  if (offset + 1 > buffer.length) {
    throw new LazorkitError(
      LazorkitErrorCode.SerializationError,
      `Buffer overflow: cannot write u8 at offset ${offset}`
    );
  }
  buffer[offset] = value & 0xff;
}

/**
 * Write u16 (little-endian) to buffer at offset
 */
function writeU16(buffer: Uint8Array, offset: number, value: number): void {
  if (offset + 2 > buffer.length) {
    throw new LazorkitError(
      LazorkitErrorCode.SerializationError,
      `Buffer overflow: cannot write u16 at offset ${offset}`
    );
  }
  buffer[offset] = value & 0xff;
  buffer[offset + 1] = (value >> 8) & 0xff;
}

/**
 * Write u32 (little-endian) to buffer at offset
 */
function writeU32(buffer: Uint8Array, offset: number, value: number): void {
  if (offset + 4 > buffer.length) {
    throw new LazorkitError(
      LazorkitErrorCode.SerializationError,
      `Buffer overflow: cannot write u32 at offset ${offset}`
    );
  }
  buffer[offset] = value & 0xff;
  buffer[offset + 1] = (value >> 8) & 0xff;
  buffer[offset + 2] = (value >> 16) & 0xff;
  buffer[offset + 3] = (value >> 24) & 0xff;
}

/**
 * Write u64 (little-endian) to buffer at offset
 */
function writeU64(buffer: Uint8Array, offset: number, value: bigint): void {
  if (offset + 8 > buffer.length) {
    throw new LazorkitError(
      LazorkitErrorCode.SerializationError,
      `Buffer overflow: cannot write u64 at offset ${offset}`
    );
  }
  let v = value;
  for (let i = 0; i < 8; i++) {
    buffer[offset + i] = Number(v & 0xffn);
    v = v >> 8n;
  }
}

/**
 * Write instruction discriminator (u16) to buffer
 */
export function writeInstructionDiscriminator(
  buffer: Uint8Array,
  discriminator: number
): void {
  writeU16(buffer, 0, discriminator);
}

/**
 * Serialize PluginRef array to bytes
 * 
 * Layout: plugin_index[2] + priority[1] + enabled[1] + padding[4] = 8 bytes per ref
 */
export function serializePluginRefs(pluginRefs: PluginRef[]): Uint8Array {
  const PLUGIN_REF_SIZE = 8; // plugin_index[2] + priority[1] + enabled[1] + padding[4]
  const buffer = new Uint8Array(pluginRefs.length * PLUGIN_REF_SIZE);

  for (let i = 0; i < pluginRefs.length; i++) {
    const ref = pluginRefs[i];
    const offset = i * PLUGIN_REF_SIZE;

    // plugin_index: 2 bytes (u16)
    writeU16(buffer, offset, ref.pluginIndex);

    // priority: 1 byte (u8)
    writeU8(buffer, offset + 2, ref.priority);

    // enabled: 1 byte (u8)
    writeU8(buffer, offset + 3, ref.enabled ? 1 : 0);

    // padding: 4 bytes (already zero-initialized)
  }

  return buffer;
}

/**
 * Serialize CreateSmartWallet instruction arguments
 * 
 * Layout: id[32] + bump[1] + wallet_bump[1] + first_authority_type[2] + 
 *         first_authority_data_len[2] + num_plugin_refs[2] + role_permission[1] + padding[1] = 43 bytes
 */
export function serializeCreateSmartWalletArgs(
  args: import('../instructions/types').CreateSmartWalletArgs
): Uint8Array {
  const buffer = new Uint8Array(48); // Aligned to 8 bytes (43 -> 48)
  let offset = 0;

  // id: 32 bytes
  if (args.id.length !== 32) {
    throw new LazorkitError(
      LazorkitErrorCode.InvalidWalletId,
      `Wallet ID must be 32 bytes, got ${args.id.length}`
    );
  }
  buffer.set(args.id, offset);
  offset += 32;

  // bump: 1 byte
  writeU8(buffer, offset, args.bump);
  offset += 1;

  // wallet_bump: 1 byte
  writeU8(buffer, offset, args.walletBump);
  offset += 1;

  // first_authority_type: 2 bytes (u16)
  writeU16(buffer, offset, args.firstAuthorityType);
  offset += 2;

  // first_authority_data_len: 2 bytes (u16)
  writeU16(buffer, offset, args.firstAuthorityDataLen);
  offset += 2;

  // num_plugin_refs: 2 bytes (u16)
  writeU16(buffer, offset, args.numPluginRefs);
  offset += 2;

  // role_permission: 1 byte (u8)
  writeU8(buffer, offset, args.rolePermission);
  offset += 1;

  // padding: 7 bytes (1 explicit + 6 implicit for align(8)) to reach 48 bytes total
  writeU8(buffer, offset, 0);
  offset += 1;
  // Implicit padding 6 bytes
  offset += 6;

  return buffer;
}

/**
 * Serialize Sign instruction arguments
 * 
 * Layout: instruction_payload_len[2] + authority_id[4] = 6 bytes
 */
export function serializeSignArgs(
  args: import('../instructions/types').SignArgs
): Uint8Array {
  const buffer = new Uint8Array(8); // Aligned to 8 bytes (6 -> 8)
  let offset = 0;

  // instruction_payload_len: 2 bytes (u16)
  writeU16(buffer, offset, args.instructionPayloadLen);
  offset += 2;

  // authority_id: 4 bytes (u32)
  writeU32(buffer, offset, args.authorityId);
  offset += 4;

  return buffer;
}

/**
 * Serialize AddAuthority instruction arguments
 * 
 * Layout: acting_authority_id[4] + new_authority_type[2] + new_authority_data_len[2] + 
 *         num_plugin_refs[2] + role_permission[1] + padding[3] = 14 bytes
 */
export function serializeAddAuthorityArgs(
  args: import('../instructions/types').AddAuthorityArgs
): Uint8Array {
  const buffer = new Uint8Array(16); // Aligned to 8 bytes (14 -> 16)
  let offset = 0;

  // acting_authority_id: 4 bytes (u32)
  writeU32(buffer, offset, args.actingAuthorityId);
  offset += 4;

  // new_authority_type: 2 bytes (u16)
  writeU16(buffer, offset, args.newAuthorityType);
  offset += 2;

  // new_authority_data_len: 2 bytes (u16)
  writeU16(buffer, offset, args.newAuthorityDataLen);
  offset += 2;

  // num_plugin_refs: 2 bytes (u16)
  writeU16(buffer, offset, args.numPluginRefs);
  offset += 2;

  // role_permission: 1 byte (u8)
  writeU8(buffer, offset, args.rolePermission);
  offset += 1;

  // padding: 3 bytes
  writeU8(buffer, offset, 0);
  writeU8(buffer, offset + 1, 0);
  writeU8(buffer, offset + 2, 0);

  return buffer;
}

/**
 * Serialize UpdateAuthority instruction arguments
 */
export function serializeUpdateAuthorityArgs(
  args: import('../instructions/types').UpdateAuthorityArgs
): Uint8Array {
  const buffer = new Uint8Array(8);
  let offset = 0;

  // acting_authority_id: 4 bytes (u32)
  writeU32(buffer, offset, args.actingAuthorityId);
  offset += 4;

  // authority_to_update_id: 4 bytes (u32)
  writeU32(buffer, offset, args.authorityToUpdateId);
  offset += 4;

  return buffer;
}

/**
 * Serialize RemoveAuthority instruction arguments
 */
export function serializeRemoveAuthorityArgs(
  args: import('../instructions/types').RemoveAuthorityArgs
): Uint8Array {
  const buffer = new Uint8Array(8);
  let offset = 0;

  // acting_authority_id: 4 bytes (u32)
  writeU32(buffer, offset, args.actingAuthorityId);
  offset += 4;

  // authority_to_remove_id: 4 bytes (u32)
  writeU32(buffer, offset, args.authorityToRemoveId);
  offset += 4;

  return buffer;
}

/**
 * Serialize AddPlugin instruction arguments
 */
export function serializeAddPluginArgs(
  args: import('../instructions/types').AddPluginArgs
): Uint8Array {
  const buffer = new Uint8Array(8); // Aligned to 8 bytes (4 -> 8)
  let offset = 0;

  // acting_authority_id: 4 bytes (u32)
  writeU32(buffer, offset, args.actingAuthorityId);
  offset += 4;

  return buffer;
}

/**
 * Serialize RemovePlugin instruction arguments
 */
export function serializeRemovePluginArgs(
  args: import('../instructions/types').RemovePluginArgs
): Uint8Array {
  const buffer = new Uint8Array(8); // Aligned to 8 bytes (6 -> 8)
  let offset = 0;

  // acting_authority_id: 4 bytes (u32)
  writeU32(buffer, offset, args.actingAuthorityId);
  offset += 4;

  // plugin_index: 2 bytes (u16)
  writeU16(buffer, offset, args.pluginIndex);
  offset += 2;

  return buffer;
}

/**
 * Serialize UpdatePlugin instruction arguments
 */
export function serializeUpdatePluginArgs(
  args: import('../instructions/types').UpdatePluginArgs
): Uint8Array {
  const buffer = new Uint8Array(8); // Aligned to 8 bytes (6 -> 8)
  let offset = 0;

  // acting_authority_id: 4 bytes (u32)
  writeU32(buffer, offset, args.actingAuthorityId);
  offset += 4;

  // plugin_index: 2 bytes (u16)
  writeU16(buffer, offset, args.pluginIndex);
  offset += 2;

  return buffer;
}

/**
 * Serialize CreateSession instruction arguments
 */
export function serializeCreateSessionArgs(
  args: import('../instructions/types').CreateSessionArgs
): Uint8Array {
  const buffer = new Uint8Array(48); // Aligned to 8 bytes (44 -> 48)
  let offset = 0;

  // authority_id: 4 bytes (u32)
  writeU32(buffer, offset, args.authorityId);
  offset += 4;

  // session_key: 32 bytes
  if (args.sessionKey.length !== 32) {
    throw new LazorkitError(
      LazorkitErrorCode.SerializationError,
      `Session key must be 32 bytes, got ${args.sessionKey.length}`
    );
  }
  buffer.set(args.sessionKey, offset);
  offset += 32;

  // Padding for u64 alignment (4 bytes)
  offset += 4;

  // duration: 8 bytes (u64)
  writeU64(buffer, offset, args.duration);
  offset += 8;

  return buffer;
}
