
import {
    Instruction,
    AccountRole,
    TransactionSigner,
    Address,
    getStructCodec,
    getU8Codec,
    getBytesCodec,
    fixCodecSize
} from "@solana/kit";
import { LAZOR_KIT_PROGRAM_ID } from "./constants";
import {
    InstructionDiscriminator,
    createWalletArgsCodec,
    addAuthorityArgsCodec,
    createSessionArgsCodec,
} from "./types";

const SYSTEM_PROGRAM = "11111111111111111111111111111111" as Address;
const RENT_SYSVAR = "SysvarRent111111111111111111111111111111111" as Address;

function getAccountMeta(address: Address, role: AccountRole, signer?: TransactionSigner) {
    return { address, role, signer };
}

export function getCreateWalletInstruction(
    input: {
        payer: TransactionSigner;
        wallet: Address;
        vault: Address;
        authority: Address;
        userSeed: Uint8Array;
        authType: number;
        authBump: number;
        authPubkey: Uint8Array;
        credentialHash: Uint8Array;
    }
): Instruction {
    const { payer, wallet, vault, authority, userSeed, authType, authBump, authPubkey, credentialHash } = input;

    // 1. Fixed args (40 bytes)
    const argsData = createWalletArgsCodec.encode({
        userSeed,
        authType,
        authBump,
        _padding: new Uint8Array(6)
    });

    // 2. Payload
    // Ed25519 (0): id_seed (32)
    // Secp256r1 (1): id_seed (32) + key (33)
    let payload: Uint8Array;
    if (authType === 0) {
        payload = authPubkey; // Should be 32 bytes
    } else {
        payload = new Uint8Array(32 + authPubkey.length);
        payload.set(credentialHash, 0);
        payload.set(authPubkey, 32);
    }

    const finalData = new Uint8Array(1 + argsData.length + payload.length);
    finalData[0] = InstructionDiscriminator.CreateWallet;
    finalData.set(argsData, 1);
    finalData.set(payload, 1 + argsData.length);

    return {
        programAddress: LAZOR_KIT_PROGRAM_ID,
        accounts: [
            getAccountMeta(payer.address, AccountRole.WRITABLE_SIGNER, payer),
            getAccountMeta(wallet, AccountRole.WRITABLE),
            getAccountMeta(vault, AccountRole.WRITABLE),
            getAccountMeta(authority, AccountRole.WRITABLE),
            getAccountMeta(SYSTEM_PROGRAM, AccountRole.READONLY),
            getAccountMeta(RENT_SYSVAR, AccountRole.READONLY),
        ],
        data: finalData
    };
}

export function getAddAuthorityInstruction(
    input: {
        payer: TransactionSigner;
        wallet: Address;
        adminAuthority: Address;
        newAuthority: Address;
        authType: number;
        newRole: number;
        authPubkey: Uint8Array;
        credentialHash: Uint8Array;
        authorizerSigner: TransactionSigner;
        signature?: Uint8Array;
    }
): Instruction {
    const { payer, wallet, adminAuthority, newAuthority, authType, newRole, authPubkey, credentialHash, authorizerSigner, signature } = input;

    const argsData = addAuthorityArgsCodec.encode({
        authorityType: authType,
        newRole,
        _padding: new Uint8Array(6)
    });

    let payload: Uint8Array;
    if (authType === 0) {
        payload = authPubkey;
    } else {
        payload = new Uint8Array(32 + authPubkey.length);
        payload.set(credentialHash, 0);
        payload.set(authPubkey, 32);
    }

    const finalData = new Uint8Array(1 + argsData.length + payload.length + (signature?.length || 0));
    finalData[0] = InstructionDiscriminator.AddAuthority;
    finalData.set(argsData, 1);
    finalData.set(payload, 1 + argsData.length);
    if (signature) {
        finalData.set(signature, 1 + argsData.length + payload.length);
    }

    const accounts = [
        getAccountMeta(payer.address, AccountRole.WRITABLE_SIGNER, payer),
        getAccountMeta(wallet, AccountRole.READONLY),
        getAccountMeta(adminAuthority, AccountRole.READONLY), // Admin PDA
        getAccountMeta(newAuthority, AccountRole.WRITABLE),
        getAccountMeta(SYSTEM_PROGRAM, AccountRole.READONLY),
        getAccountMeta(RENT_SYSVAR, AccountRole.READONLY),
        getAccountMeta(authorizerSigner.address, AccountRole.READONLY_SIGNER, authorizerSigner),
    ];

    return {
        programAddress: LAZOR_KIT_PROGRAM_ID,
        accounts,
        data: finalData
    };
}

export function getCreateSessionInstruction(
    input: {
        payer: TransactionSigner;
        wallet: Address;
        adminAuthority: Address;
        session: Address;
        sessionKey: Uint8Array;
        expiresAt: bigint;
        authorizerSigner: TransactionSigner;
        signature?: Uint8Array;
    }
): Instruction {
    const { payer, wallet, adminAuthority, session, sessionKey, expiresAt, authorizerSigner, signature } = input;

    // Args: session_key(32) + expires_at(8)
    const argsData = createSessionArgsCodec.encode({
        sessionKey,
        expiresAt,
    });

    const finalData = new Uint8Array(1 + argsData.length + (signature?.length || 0));
    finalData[0] = InstructionDiscriminator.CreateSession;
    finalData.set(argsData, 1);
    if (signature) {
        finalData.set(signature, 1 + argsData.length);
    }

    const accounts = [
        getAccountMeta(payer.address, AccountRole.WRITABLE_SIGNER, payer),
        getAccountMeta(wallet, AccountRole.READONLY),
        getAccountMeta(adminAuthority, AccountRole.WRITABLE), // Admin PDA
        getAccountMeta(session, AccountRole.WRITABLE),
        getAccountMeta(SYSTEM_PROGRAM, AccountRole.READONLY),
        getAccountMeta(RENT_SYSVAR, AccountRole.READONLY),
        getAccountMeta(authorizerSigner.address, AccountRole.READONLY_SIGNER, authorizerSigner),
    ];

    return {
        programAddress: LAZOR_KIT_PROGRAM_ID,
        accounts,
        data: finalData
    };
}

export function getExecuteInstruction(
    input: {
        payer: TransactionSigner;
        wallet: Address;
        authority: Address;
        vault: Address;
        packedInstructions: Uint8Array;
        sysvarInstructions?: Address;
        authorizerSigner?: TransactionSigner; // Actual key signing (e.g. for session)
        signature?: Uint8Array;
    }
): Instruction {
    const { payer, wallet, authority, vault, packedInstructions, sysvarInstructions, authorizerSigner, signature } = input;

    // Data format for Execute: [disc(1)][packed_data]
    // packed_data already starts with a 1-byte count prefix (from packCompactInstructions)
    const finalData = new Uint8Array(1 + packedInstructions.length + (signature?.length || 0));
    finalData[0] = InstructionDiscriminator.Execute;
    finalData.set(packedInstructions, 1);
    if (signature) {
        finalData.set(signature, packedInstructions.length + 1);
    }

    const accounts = [
        getAccountMeta(payer.address, AccountRole.WRITABLE_SIGNER, payer),
        getAccountMeta(wallet, AccountRole.READONLY),
        getAccountMeta(authority, AccountRole.WRITABLE), // Contract enforces writable even for sessions
        getAccountMeta(vault, AccountRole.WRITABLE), // Vault MUST be writable to send funds
    ];
    if (sysvarInstructions) {
        accounts.push(getAccountMeta(sysvarInstructions, AccountRole.READONLY));
    }
    if (authorizerSigner) {
        accounts.push(getAccountMeta(authorizerSigner.address, AccountRole.READONLY_SIGNER, authorizerSigner));
    }

    return {
        programAddress: LAZOR_KIT_PROGRAM_ID,
        accounts,
        data: finalData
    };
}

const transferOwnershipArgsCodec = getStructCodec([
    ['authType', getU8Codec()],
    ['authPubkey', fixCodecSize(getBytesCodec(), 32)],
    ['credentialHash', fixCodecSize(getBytesCodec(), 32)],
]);

export function getTransferOwnershipInstruction(
    input: {
        payer: TransactionSigner;
        wallet: Address;
        currentOwnerAuthority: Address;
        newOwnerAuthority: Address;
        authType: number;
        authPubkey: Uint8Array;
        credentialHash: Uint8Array;
        authorizerSigner: TransactionSigner;
        signature?: Uint8Array;
    }
): Instruction {
    const { payer, wallet, currentOwnerAuthority, newOwnerAuthority, authType, authPubkey, credentialHash, authorizerSigner, signature } = input;

    const argsData = transferOwnershipArgsCodec.encode({
        authType,
        authPubkey,
        credentialHash,
    });

    const finalData = new Uint8Array(1 + argsData.length + (signature?.length || 0));
    finalData[0] = InstructionDiscriminator.TransferOwnership;
    finalData.set(argsData, 1);
    if (signature) {
        finalData.set(signature, 1 + argsData.length);
    }

    const accounts = [
        getAccountMeta(payer.address, AccountRole.WRITABLE_SIGNER, payer),
        getAccountMeta(wallet, AccountRole.READONLY),
        getAccountMeta(currentOwnerAuthority, AccountRole.WRITABLE),
        getAccountMeta(newOwnerAuthority, AccountRole.WRITABLE),
        getAccountMeta(SYSTEM_PROGRAM, AccountRole.READONLY),
        getAccountMeta(RENT_SYSVAR, AccountRole.READONLY),
        getAccountMeta(authorizerSigner.address, AccountRole.READONLY_SIGNER, authorizerSigner),
    ];

    return {
        programAddress: LAZOR_KIT_PROGRAM_ID,
        accounts,
        data: finalData
    };
}

export function getRemoveAuthorityInstruction(
    input: {
        payer: TransactionSigner;
        wallet: Address;
        adminAuthority: Address;
        targetAuthority: Address;
        refundDestination: Address;
        authorizerSigner: TransactionSigner;
        signature?: Uint8Array;
    }
): Instruction {
    const { payer, wallet, adminAuthority, targetAuthority, refundDestination, authorizerSigner, signature } = input;

    const finalData = new Uint8Array(1 + (signature?.length || 0));
    finalData[0] = InstructionDiscriminator.RemoveAuthority;
    if (signature) {
        finalData.set(signature, 1);
    }

    const accounts = [
        getAccountMeta(payer.address, AccountRole.WRITABLE_SIGNER, payer),
        getAccountMeta(wallet, AccountRole.READONLY),
        getAccountMeta(adminAuthority, AccountRole.WRITABLE),
        getAccountMeta(targetAuthority, AccountRole.WRITABLE),
        getAccountMeta(refundDestination, AccountRole.WRITABLE),
        getAccountMeta(authorizerSigner.address, AccountRole.READONLY_SIGNER, authorizerSigner),
    ];

    return {
        programAddress: LAZOR_KIT_PROGRAM_ID,
        accounts,
        data: finalData
    };
}
