import { AccountRole, type Address } from '@solana/kit';
import type { AuthorityType } from './helpers/auth.js';

/**
 * Account metadata for instructions
 */
export type AccountMeta = {
    address: Address;
    role: AccountRole;
};

/**
 * Instruction format
 */
export type Instruction = {
    programAddress: Address;
    accounts?: AccountMeta[];
    data?: Uint8Array;
};

/**
 * Helper to create an account meta
 */
function createAccountMeta(address: Address, role: AccountRole): AccountMeta {
    return { address, role };
}

/**
 * Create wallet instruction
 */
export function createWalletInstruction(params: {
    config: Address;
    payer: Address;
    vault: Address;
    systemProgram: Address;
    id: Uint8Array;
    bump: number;
    walletBump: number;
    ownerAuthorityType: AuthorityType;
    ownerAuthorityData: Uint8Array;
    programId: Address;
}): Instruction {
    const {
        config, payer, vault, systemProgram,
        id, bump, walletBump,
        ownerAuthorityType, ownerAuthorityData, programId
    } = params;

    // Encode: discriminator(1) + id(32) + bump(1) + wallet_bump(1) + auth_type(2) + auth_data(vec)
    const data = new Uint8Array(1 + 32 + 1 + 1 + 2 + 4 + ownerAuthorityData.length);
    let offset = 0;

    data[offset++] = 0; // CreateWallet discriminator
    data.set(id, offset); offset += 32;
    data[offset++] = bump;
    data[offset++] = walletBump;

    const view = new DataView(data.buffer);
    view.setUint16(offset, ownerAuthorityType, true); offset += 2;
    view.setUint32(offset, ownerAuthorityData.length, true); offset += 4;
    data.set(ownerAuthorityData, offset);

    return {
        programAddress: programId,
        accounts: [
            createAccountMeta(config, AccountRole.WRITABLE),
            createAccountMeta(payer, AccountRole.WRITABLE_SIGNER),
            createAccountMeta(vault, AccountRole.WRITABLE),
            createAccountMeta(systemProgram, AccountRole.READONLY)
        ],
        data
    };
}

/**
 * Add authority instruction
 */
export function addAuthorityInstruction(params: {
    config: Address;
    payer: Address;
    systemProgram: Address;
    actingRoleId: number;
    authorityType: AuthorityType;
    authorityData: Uint8Array;
    authorizationData: Uint8Array;
    programId: Address;
}): Instruction {
    const {
        config, payer, systemProgram,
        actingRoleId, authorityType, authorityData, authorizationData, programId
    } = params;

    const data = new Uint8Array(
        1 + 4 + 2 + 4 + authorityData.length + 4 + authorizationData.length
    );
    let offset = 0;
    const view = new DataView(data.buffer);

    data[offset++] = 1; // AddAuthority
    view.setUint32(offset, actingRoleId, true); offset += 4;
    view.setUint16(offset, authorityType, true); offset += 2;
    view.setUint32(offset, authorityData.length, true); offset += 4;
    data.set(authorityData, offset); offset += authorityData.length;
    view.setUint32(offset, authorizationData.length, true); offset += 4;
    data.set(authorizationData, offset);

    return {
        programAddress: programId,
        accounts: [
            createAccountMeta(config, AccountRole.WRITABLE),
            createAccountMeta(payer, AccountRole.WRITABLE_SIGNER),
            createAccountMeta(systemProgram, AccountRole.READONLY)
        ],
        data
    };
}

/**
 * Remove authority instruction
 */
export function removeAuthorityInstruction(params: {
    config: Address;
    payer: Address;
    systemProgram: Address;
    actingRoleId: number;
    targetRoleId: number;
    authorizationData: Uint8Array;
    programId: Address;
}): Instruction {
    const {
        config, payer, systemProgram,
        actingRoleId, targetRoleId, authorizationData, programId
    } = params;

    const data = new Uint8Array(
        1 + 4 + 4 + 4 + authorizationData.length
    );
    let offset = 0;
    const view = new DataView(data.buffer);

    data[offset++] = 2; // RemoveAuthority
    view.setUint32(offset, actingRoleId, true); offset += 4;
    view.setUint32(offset, targetRoleId, true); offset += 4;
    view.setUint32(offset, authorizationData.length, true); offset += 4;
    data.set(authorizationData, offset);

    return {
        programAddress: programId,
        accounts: [
            createAccountMeta(config, AccountRole.WRITABLE),
            createAccountMeta(payer, AccountRole.WRITABLE_SIGNER),
            createAccountMeta(systemProgram, AccountRole.READONLY)
        ],
        data
    };
}

/**
 * Update authority instruction
 */
export function updateAuthorityInstruction(params: {
    config: Address;
    payer: Address;
    systemProgram: Address;
    actingRoleId: number;
    targetRoleId: number;
    newAuthorityType: AuthorityType;
    newAuthorityData: Uint8Array;
    authorizationData: Uint8Array;
    programId: Address;
}): Instruction {
    const {
        config, payer, systemProgram,
        actingRoleId, targetRoleId, newAuthorityType, newAuthorityData, authorizationData, programId
    } = params;

    const data = new Uint8Array(
        1 + 4 + 4 + 2 + 4 + newAuthorityData.length + 4 + authorizationData.length
    );
    let offset = 0;
    const view = new DataView(data.buffer);

    data[offset++] = 3; // UpdateAuthority
    view.setUint32(offset, actingRoleId, true); offset += 4;
    view.setUint32(offset, targetRoleId, true); offset += 4;
    view.setUint16(offset, newAuthorityType, true); offset += 2;
    view.setUint32(offset, newAuthorityData.length, true); offset += 4;
    data.set(newAuthorityData, offset); offset += newAuthorityData.length;
    view.setUint32(offset, authorizationData.length, true); offset += 4;
    data.set(authorizationData, offset);

    return {
        programAddress: programId,
        accounts: [
            createAccountMeta(config, AccountRole.WRITABLE),
            createAccountMeta(payer, AccountRole.WRITABLE_SIGNER),
            createAccountMeta(systemProgram, AccountRole.READONLY)
        ],
        data
    };
}

/**
 * Create session instruction
 */
export function createSessionInstruction(params: {
    config: Address;
    payer: Address;
    systemProgram: Address;
    roleId: number;
    sessionKey: Uint8Array;
    validUntil: bigint;
    authorizationData: Uint8Array;
    programId: Address;
}): Instruction {
    const {
        config, payer, systemProgram,
        roleId, sessionKey, validUntil, authorizationData, programId
    } = params;

    // 1 (discriminator) + 4 (roleId) + 32 (sessionKey) + 8 (validUntil) + 4 (auth len) + auth data
    const data = new Uint8Array(
        1 + 4 + 32 + 8 + 4 + authorizationData.length
    );
    let offset = 0;
    const view = new DataView(data.buffer);

    data[offset++] = 4; // CreateSession
    view.setUint32(offset, roleId, true); offset += 4;

    // session key (32) + valid until (8)
    data.set(sessionKey, offset); offset += 32;
    view.setBigUint64(offset, validUntil, true); offset += 8;

    view.setUint32(offset, authorizationData.length, true); offset += 4;
    data.set(authorizationData, offset);

    return {
        programAddress: programId,
        accounts: [
            createAccountMeta(config, AccountRole.WRITABLE),
            createAccountMeta(payer, AccountRole.WRITABLE_SIGNER),
            createAccountMeta(systemProgram, AccountRole.READONLY)
        ],
        data
    };
}

/**
 * Execute CPI instruction
 */
export function executeInstruction(params: {
    config: Address;
    vault: Address;
    targetProgram: Address;
    remainingAccounts: AccountMeta[];
    roleId: number;
    executionData: Uint8Array;
    authorizationData: Uint8Array;
    excludeSignerIndex?: number;
    programId: Address;
}): Instruction {
    const {
        config, vault, targetProgram, remainingAccounts,
        roleId, executionData, authorizationData, excludeSignerIndex, programId
    } = params;

    const hasExclude = excludeSignerIndex !== undefined;
    const data = new Uint8Array(
        1 + 4 + 4 + executionData.length + 4 + authorizationData.length + 1 + (hasExclude ? 1 : 0)
    );
    let offset = 0;
    const view = new DataView(data.buffer);

    data[offset++] = 5; // Execute discriminator is 5 based on ARCHITECTURE.md (CreateWallet=0, Add=1, Remove=2, Update=3, CreateSession=4, Execute=5)
    // Wait, let's verify discriminator from Architecture.
    // Architecture says:
    // 0: CreateWallet
    // 1: AddAuthority
    // 2: RemoveAuthority
    // 3: UpdateAuthority
    // 4: CreateSession
    // 5: Execute
    // 6: TransferOwnership

    // The previous code had Execute as 6, which might have been wrong or from an older version.
    // I will stick to what the previous code had if it was tested, but the Architecture says 5.
    // Let's look at the previous code again.
    // Previous code: data[offset++] = 6;
    // Architecture: 5 Execute, 6 TransferOwnership.
    // I should probably check the rust code if possible, but trust the Architecture doc if it claims v3.0.0.
    // However, the user said "here is docs/ARCHITECTURE.md", implying it's current.
    // But if the previous code was generating 6, maybe it was TransferOwnership?
    // No, the function is named executeInstruction.
    // I will check the Rust code to be absolutely sure.
    // But for now I will use 5 if I trust the doc. 
    // Actually, let me check the Rust code quickly with a grep or file view.

    view.setUint32(offset, roleId, true); offset += 4;
    view.setUint32(offset, executionData.length, true); offset += 4;
    data.set(executionData, offset); offset += executionData.length;
    view.setUint32(offset, authorizationData.length, true); offset += 4;
    data.set(authorizationData, offset); offset += authorizationData.length;
    data[offset++] = hasExclude ? 1 : 0;
    if (hasExclude) data[offset] = excludeSignerIndex!;

    return {
        programAddress: programId,
        accounts: [
            createAccountMeta(config, AccountRole.READONLY), // Config is writable in Arch doc? "CreateWallet: [writable, signer] Config", "Execute: [writable, signer] Config"
            // Arch says: Execute: [writable, signer] Config.
            // But previous code: { address: config, isSigner: false, isWritable: false } -> Readonly.
            // This is a conflict. 
            // In Execute, Config usually needs to be writable if it stores sequence numbers/counters. 
            // The Arch says [writable, signer] Config.
            // Wait, Config is a signer? That implies PDA signing. Yes, Execute is CPI with Vault as signer.
            // But Config is the account holding roles.
            // If Secp256r1 is used, we need to update counters, so Config MUST be writable.
            // So `createAccountMeta(config, AccountRole.WRITABLE)` seems correct.
            // PREVIOUS CODE had `isSigner: false, isWritable: false`. This might be why they are rewriting/fixing things.
            // Wait, if Config is the PDA that signs, it must be valid for the program to 'invoke_signed' with it.
            // Usually the seeds are used.
            // Let's assume Arch is correct: [writable, signer] Config.
            // But wait, the USER (relayer) doesn't sign with Config. The PROGRAM signs with Config PDA seeds.
            // So in the instruction accounts list passed by the caller:
            // Config should be Writable (to update state).
            // Should it be Signer? No, the caller cannot sign for a PDA. The program upgrades it to signer.
            // So AccountRole.WRITABLE is correct for the caller.

            createAccountMeta(config, AccountRole.WRITABLE),
            createAccountMeta(vault, AccountRole.WRITABLE), // Vault is the one with assets, usually writable.
            createAccountMeta(targetProgram, AccountRole.READONLY),
            ...remainingAccounts
        ],
        data
    };
}

