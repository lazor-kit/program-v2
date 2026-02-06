import {
    Address,
    address,
    getProgramDerivedAddress,
} from "@solana/addresses";
import {
    AccountMeta,
    Instruction,
} from "@solana/instructions";
import {
    getBytesEncoder,
    getStructEncoder,
    getU8Encoder,
    getU64Encoder,
    fixEncoderSize,
    getU32Encoder, // Unused but imported to avoid lint error if needed
} from "@solana/codecs";
import { LAZORKIT_PROGRAM_ID } from "./constants";
import { findWalletPDA, findVaultPDA, findAuthorityPDA, findSessionPDA } from "./utils";
import { AuthType, Role } from "./types";

// Discriminators (u8)
const DISC_CREATE_WALLET = 0;
const DISC_ADD_AUTHORITY = 1;
const DISC_REMOVE_AUTHORITY = 2;
const DISC_TRANSFER_OWNERSHIP = 3;
const DISC_EXECUTE = 4;
const DISC_CREATE_SESSION = 5;

// =========================================================================
// Encoders (Headers)
// =========================================================================

// CreateWallet: [user_seed:32][type:1][bump:1][padding:6] = 40 bytes
const createWalletHeaderEncoder = getStructEncoder([
    ['userSeed', fixEncoderSize(getBytesEncoder(), 32)],
    ['authType', getU8Encoder()],
    ['authBump', getU8Encoder()],
    ['padding', fixEncoderSize(getBytesEncoder(), 6)],
]);

// AddAuthority: [type:1][role:1][padding:6] = 8 bytes
const addAuthorityHeaderEncoder = getStructEncoder([
    ['authType', getU8Encoder()],
    ['newRole', getU8Encoder()],
    ['padding', fixEncoderSize(getBytesEncoder(), 6)],
]);

// TransferOwnership: [type:1] = 1 byte
const transferOwnershipHeaderEncoder = getStructEncoder([
    ['authType', getU8Encoder()],
]);

// CreateSession: [sessionKey:32][expiresAt:8] = 40 bytes
const createSessionHeaderEncoder = getStructEncoder([
    ['sessionKey', fixEncoderSize(getBytesEncoder(), 32)],
    ['expiresAt', getU64Encoder()],
]);

// Execute: [serialized_instructions] (Just raw bytes, manual length prefix if required?)
// Processor logic for execute: `let instructions = remaining_data`. 
// Doesn't seem to enforce header. Checking usage... usually just bytes.
// But wait, if we use `getStructEncoder` we might strictly encode.
// Let's use simple manual packing for execute.

// =========================================================================
// Helper to encode Payload
// =========================================================================
function encodePayload(authType: AuthType, pubkey: Uint8Array, hash: Uint8Array): Uint8Array {
    if (authType === AuthType.Ed25519) {
        // [pubkey: 32]
        const encoded = fixEncoderSize(getBytesEncoder(), 32).encode(pubkey.slice(0, 32));
        return new Uint8Array(encoded);
    } else {
        // Secp256r1: [hash: 32][pubkey: variable]
        const hashEncoded = fixEncoderSize(getBytesEncoder(), 32).encode(hash);
        const keyEncoded = getBytesEncoder().encode(pubkey);
        const payload = new Uint8Array(hashEncoded.length + keyEncoded.length);
        payload.set(hashEncoded);
        payload.set(keyEncoded, hashEncoded.length);
        return payload;
    }
}

// =========================================================================
// Builder Functions
// =========================================================================

export async function createWalletInstruction(
    payer: Address,
    userSeed: Uint8Array,
    authType: AuthType,
    authPubkey: Uint8Array,
    credentialHash: Uint8Array,
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<Instruction> {
    const [walletPda] = await findWalletPDA(userSeed, programId);
    const [vaultPda] = await findVaultPDA(walletPda, programId);

    const isEd25519 = authType === AuthType.Ed25519;
    // Derive Authority PDA
    const seedForAuth = isEd25519 ? authPubkey.slice(0, 32) : credentialHash;
    const [authorityPda, authBump] = await findAuthorityPDA(walletPda, seedForAuth, programId);

    // 1. Header (40 bytes)
    const header = createWalletHeaderEncoder.encode({
        userSeed,
        authType,
        authBump,
        padding: new Uint8Array(6),
    });

    // 2. Payload
    const payload = encodePayload(authType, authPubkey, credentialHash);

    // Data: [Discr: 1] + [Header: 40] + [Payload]
    const data = new Uint8Array(1 + header.length + payload.length);
    data[0] = DISC_CREATE_WALLET;
    data.set(header, 1);
    data.set(payload, 1 + header.length);

    return {
        programAddress: programId,
        accounts: [
            { address: payer, role: 3 }, // signer, writable
            { address: walletPda, role: 1 }, // writable
            { address: vaultPda, role: 1 }, // writable
            { address: authorityPda, role: 1 }, // writable
            { address: address("11111111111111111111111111111111"), role: 0 }, // system program
            { address: address("SysvarRent111111111111111111111111111111111"), role: 0 }, // rent sysvar
        ],
        data,
    };
}

export async function createExecuteInstruction(
    payer: Address,
    wallet: Address,
    authorityPda: Address,
    instructions: Uint8Array,
    remainingAccounts: AccountMeta[] = [],
    authoritySigner?: Address, // If different from payer
    programId: Address = LAZORKIT_PROGRAM_ID,
    isSecp256r1: boolean = false
): Promise<Instruction> {
    const [vaultPda] = await findVaultPDA(wallet, programId);

    // For Execute, data is: [Discr: 1] + [instructions_bytes]
    // The processor just consuming instructions from bytes?
    // Let's assume passed as remainder.
    // NOTE: If instructions need a length prefix (u32), verify processor.
    // Based on previous code assumption: just bytes.

    const data = new Uint8Array(1 + instructions.length);
    data[0] = DISC_EXECUTE;
    data.set(instructions, 1);

    const accounts: AccountMeta[] = [
        { address: payer, role: 3 }, // signer, writable (payer)
        { address: wallet, role: 0 }, // readonly
        { address: authorityPda, role: 1 }, // writable (Authority PDA)
        { address: vaultPda, role: 1 }, // writable (Vault signs via CPI)
    ];

    accounts.push(...remainingAccounts);

    if (authoritySigner && authoritySigner !== payer) {
        accounts.push({ address: authoritySigner, role: 2 }); // signer, readonly
    }

    if (isSecp256r1) {
        accounts.push({ address: address("Sysvar1nstructions1111111111111111111111111"), role: 0 });
    }

    return {
        programAddress: programId,
        accounts,
        data,
    };
}

export async function addAuthorityInstruction(
    payer: Address,
    wallet: Address,
    adminAuthorityPda: Address,
    newAuthType: AuthType,
    newPubkey: Uint8Array,
    newHash: Uint8Array,
    newAuthRole: Role,
    adminSigner?: Address,
    authPayload: Uint8Array = new Uint8Array(0),
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<Instruction> {
    const seedForAuth = newAuthType === AuthType.Ed25519 ? newPubkey.slice(0, 32) : newHash;
    const [newAuthPda] = await findAuthorityPDA(wallet, seedForAuth, programId);

    // 1. Header (8 bytes)
    const header = addAuthorityHeaderEncoder.encode({
        authType: newAuthType,
        newRole: newAuthRole,
        padding: new Uint8Array(6)
    });

    // 2. Payload (New Authority Data)
    const payload = encodePayload(newAuthType, newPubkey, newHash);

    // Data: [Discr: 1] + [Header: 8] + [Payload] + [AdminAuthPayload]
    // Note: Admin authentication payload comes LAST.
    const data = new Uint8Array(1 + header.length + payload.length + authPayload.length);
    data[0] = DISC_ADD_AUTHORITY;
    data.set(header, 1);
    data.set(payload, 1 + header.length);
    data.set(authPayload, 1 + header.length + payload.length);

    const accounts: AccountMeta[] = [
        { address: payer, role: 3 },
        { address: wallet, role: 0 },
        { address: adminAuthorityPda, role: 1 },
        { address: newAuthPda, role: 1 }, // writable
        { address: address("11111111111111111111111111111111"), role: 0 },
    ];

    if (adminSigner) {
        accounts.push({ address: adminSigner, role: 2 }); // signer
    }

    return {
        programAddress: programId,
        accounts,
        data
    };
}

export async function createSessionInstruction(
    payer: Address,
    wallet: Address,
    authorizerPda: Address,
    sessionKey: Uint8Array,
    expiresAt: bigint,
    authorizerSigner?: Address,
    authPayload: Uint8Array = new Uint8Array(0),
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<Instruction> {
    const [sessionPda] = await findSessionPDA(wallet, sessionKey, programId);

    // Header (40 bytes)
    const header = createSessionHeaderEncoder.encode({
        sessionKey,
        expiresAt
    });

    // Data: [Discr: 1] + [Header: 40] + [AuthPayload]
    const data = new Uint8Array(1 + header.length + authPayload.length);
    data[0] = DISC_CREATE_SESSION;
    data.set(header, 1);
    data.set(authPayload, 1 + header.length);

    const accounts: AccountMeta[] = [
        { address: payer, role: 3 },
        { address: wallet, role: 0 },
        { address: authorizerPda, role: 1 },
        { address: sessionPda, role: 1 },
        { address: address("11111111111111111111111111111111"), role: 0 },
        { address: address("SysvarRent111111111111111111111111111111111"), role: 0 }, // rent sysvar
    ];

    if (authorizerSigner) {
        accounts.push({ address: authorizerSigner, role: 2 });
    }

    return {
        programAddress: programId,
        accounts,
        data
    };
}

export async function removeAuthorityInstruction(
    payer: Address,
    wallet: Address,
    adminAuthorityPda: Address,
    targetAuthorityPda: Address,
    refundDestination: Address,
    adminSigner?: Address,
    authPayload: Uint8Array = new Uint8Array(0),
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<Instruction> {
    // Data: [Discr: 1] + [AuthPayload]
    const data = new Uint8Array(1 + authPayload.length);
    data[0] = DISC_REMOVE_AUTHORITY;
    data.set(authPayload, 1);

    const accounts: AccountMeta[] = [
        { address: payer, role: 3 },
        { address: wallet, role: 0 },
        { address: adminAuthorityPda, role: 1 },
        { address: targetAuthorityPda, role: 1 },
        { address: refundDestination, role: 1 },
    ];

    if (adminSigner) {
        accounts.push({ address: adminSigner, role: 2 });
    }

    return {
        programAddress: programId,
        accounts,
        data
    };
}

export async function transferOwnershipInstruction(
    payer: Address,
    wallet: Address,
    currentOwnerPda: Address,
    newType: AuthType,
    newPubkey: Uint8Array,
    newHash: Uint8Array,
    ownerSigner?: Address,
    authPayload: Uint8Array = new Uint8Array(0),
    programId: Address = LAZORKIT_PROGRAM_ID
): Promise<Instruction> {
    const seedForAuth = newType === AuthType.Ed25519 ? newPubkey.slice(0, 32) : newHash;
    const [newOwnerPda] = await findAuthorityPDA(wallet, seedForAuth, programId);

    // Header (1 byte)
    const header = transferOwnershipHeaderEncoder.encode({
        authType: newType
    });

    // 2. Payload (New Owner Data)
    const payload = encodePayload(newType, newPubkey, newHash);

    // Data: [Discr: 1] + [Header: 1] + [Payload] + [AuthPayload]
    const data = new Uint8Array(1 + header.length + payload.length + authPayload.length);
    data[0] = DISC_TRANSFER_OWNERSHIP;
    data.set(header, 1);
    data.set(payload, 1 + header.length);
    data.set(authPayload, 1 + header.length + payload.length);

    const accounts: AccountMeta[] = [
        { address: payer, role: 3 },
        { address: wallet, role: 0 },
        { address: currentOwnerPda, role: 1 }, // writable
        { address: newOwnerPda, role: 1 }, // writable
        { address: address("11111111111111111111111111111111"), role: 0 }, // System Program
    ];

    if (ownerSigner) {
        accounts.push({ address: ownerSigner, role: 2 });
    }

    return {
        programAddress: programId,
        accounts,
        data
    };
}
