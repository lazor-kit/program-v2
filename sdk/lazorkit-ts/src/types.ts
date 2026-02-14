
import {
    getStructCodec,
    getU8Codec,
    getU64Codec,
    getU32Codec,
    getArrayCodec,
    getBytesCodec,
    fixCodecSize,
    Codec,
} from "@solana/kit";

// Helper for type inference
export type CodecType<T> = T extends Codec<infer U> ? U : never;

// Re-export common types
export type { Address } from "@solana/kit";

// --- Account Codecs ---

export const walletAccountCodec = getStructCodec([
    ["discriminator", getU8Codec()],
    ["bump", getU8Codec()],
    ["version", getU8Codec()],
    ["_padding", fixCodecSize(getBytesCodec(), 5)],
]);
export type WalletAccount = CodecType<typeof walletAccountCodec>;

export const authorityAccountHeaderCodec = getStructCodec([
    ["discriminator", getU8Codec()],
    ["authorityType", getU8Codec()],
    ["role", getU8Codec()],
    ["bump", getU8Codec()],
    ["version", getU8Codec()],
    ["_padding", fixCodecSize(getBytesCodec(), 3)],
    ["counter", getU64Codec()],
    ["wallet", fixCodecSize(getBytesCodec(), 32)],
]);
export type AuthorityAccountHeader = CodecType<typeof authorityAccountHeaderCodec>;

export const sessionAccountCodec = getStructCodec([
    ["discriminator", getU8Codec()],
    ["bump", getU8Codec()],
    ["version", getU8Codec()],
    ["_padding", fixCodecSize(getBytesCodec(), 5)],
    ["wallet", fixCodecSize(getBytesCodec(), 32)],
    ["sessionKey", fixCodecSize(getBytesCodec(), 32)],
    ["expiresAt", getU64Codec()],
]);
export type SessionAccount = CodecType<typeof sessionAccountCodec>;


// --- Instruction Argument Codecs (Internal Structs) ---

/**
 * Align with contract's repr(C) CreateWalletArgs
 */
export const createWalletArgsCodec = getStructCodec([
    ["userSeed", fixCodecSize(getBytesCodec(), 32)],
    ["authType", getU8Codec()],
    ["authBump", getU8Codec()],
    ["_padding", fixCodecSize(getBytesCodec(), 6)],
]);
export type CreateWalletArgs = CodecType<typeof createWalletArgsCodec>;

/**
 * Align with contract's repr(C) AddAuthorityArgs
 */
export const addAuthorityArgsCodec = getStructCodec([
    ["authorityType", getU8Codec()],
    ["newRole", getU8Codec()],
    ["_padding", fixCodecSize(getBytesCodec(), 6)],
]);
export type AddAuthorityArgs = CodecType<typeof addAuthorityArgsCodec>;

/**
 * Align with contract's repr(C) CreateSessionArgs
 */
export const createSessionArgsCodec = getStructCodec([
    ["sessionKey", fixCodecSize(getBytesCodec(), 32)],
    ["expiresAt", getU64Codec()],
]);
export type CreateSessionArgs = CodecType<typeof createSessionArgsCodec>;

// --- Discriminators ---

export enum InstructionDiscriminator {
    CreateWallet = 0,
    AddAuthority = 1,
    RemoveAuthority = 2,
    TransferOwnership = 3,
    Execute = 4,
    CreateSession = 5,
}
