/**
 * LazorClient — Thin compatibility wrapper over manual instruction encoding.
 * 
 * IMPLEMENTATION NOTE:
 * We manually encode instruction data here because the generated Codama encoders
 * use `u32` length prefixes for `bytes` fields, but the LazorKit contract expects
 * raw fixed-size byte arrays (C-struct style) with specific alignment padding.
 * 
 * Using the generated `get*Instruction` functions causes mismatched instruction data,
 * leading to "InvalidInstructionData" errors in the contract.
 */

import {
    getCreateWalletInstruction,
    getAddAuthorityInstruction,
    getRemoveAuthorityInstruction,
    getTransferOwnershipInstruction,
    getCreateSessionInstruction
} from "../generated";

import {
    getAddressEncoder,
    type Address,
    type TransactionSigner,
    type ReadonlyUint8Array,
    type AccountMeta,
    type AccountSignerMeta,
    type ProgramDerivedAddress,
    AccountRole,
    upgradeRoleToSigner,
} from "@solana/kit";

import {
    LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
} from "../generated";

import {
    fetchWalletAccount,
    fetchAuthorityAccount,
    fetchSessionAccount,
    type WalletAccount,
    type AuthorityAccount,
    type SessionAccount,
} from "../generated/accounts";

import { packCompactInstructions, type CompactInstruction } from "./packing";
import { findAuthorityPda } from "./pdas";

// Valid address input types
type AddressLike = Address | ProgramDerivedAddress;

// Helper to resolve AddressLike to Address string
function resolveAddress(addr: AddressLike | TransactionSigner): Address {
    if (Array.isArray(addr)) return addr[0];
    if (typeof addr === 'object' && 'address' in addr) return (addr as TransactionSigner).address;
    return addr as Address;
}

// Helper to create account meta
function meta(
    address: AddressLike | TransactionSigner,
    role: "r" | "w" | "rs" | "ws" | "s",
): AccountMeta | AccountSignerMeta {
    const addr = resolveAddress(address);
    const isSignerObj = typeof address === 'object' && ('signTransaction' in address || 'signTransactions' in address) && !Array.isArray(address);

    // Determine base role (Readonly or Writable)
    let accountRole = role.includes('w') ? AccountRole.WRITABLE : AccountRole.READONLY;

    // Upgrade to signer if needed
    // We strictly follow the requested role. 
    // If 's' is in role, we force it to be a signer role.
    if (role.includes('s')) {
        accountRole = upgradeRoleToSigner(accountRole);
    }

    return {
        address: addr,
        role: accountRole,
        ...(role.includes('s') && isSignerObj ? { signer: address as TransactionSigner } : {}),
    } as any;
}

export class LazorClient {
    constructor(private rpc: any) { }

    private getAuthPayload(authType: number, authPubkey: ReadonlyUint8Array, credentialHash: ReadonlyUint8Array): Uint8Array {
        if (authType === 1) { // Secp256r1
            // 32 bytes hash + 33 bytes key
            const payload = new Uint8Array(65);
            payload.set(credentialHash, 0);
            payload.set(authPubkey, 32);
            return payload;
        } else { // Ed25519
            // 32 bytes key
            return new Uint8Array(authPubkey);
        }
    }

    // Helper to strip the 4-byte length prefix added by Codama for 'bytes' type
    private stripPayloadPrefix(data: ReadonlyUint8Array, payloadOffset: number): Uint8Array {
        // [Head][Prefix(4)][Payload] -> [Head][Payload]
        // data.length = payloadOffset + 4 + payloadLen
        const payloadLen = data.length - payloadOffset - 4;
        if (payloadLen < 0) return new Uint8Array(data); // Should not happen if correctly generated

        const fixed = new Uint8Array(data.length - 4);
        fixed.set(data.slice(0, payloadOffset), 0);
        fixed.set(data.slice(payloadOffset + 4), payloadOffset);
        return fixed;
    }

    createWallet(params: {
        payer: TransactionSigner;
        wallet: AddressLike;
        vault: AddressLike;
        authority: AddressLike;
        config: AddressLike;
        treasuryShard: AddressLike;
        userSeed: ReadonlyUint8Array;
        authType: number;
        authBump?: number;
        authPubkey: ReadonlyUint8Array;
        credentialHash: ReadonlyUint8Array;
    }) {
        const authBump = params.authBump || 0;
        const padding = new Uint8Array(6).fill(0);
        const payload = this.getAuthPayload(params.authType, params.authPubkey, params.credentialHash);

        const instruction = getCreateWalletInstruction({
            payer: params.payer,
            wallet: resolveAddress(params.wallet),
            vault: resolveAddress(params.vault),
            authority: resolveAddress(params.authority),
            config: resolveAddress(params.config),
            treasuryShard: resolveAddress(params.treasuryShard),
            userSeed: params.userSeed,
            authType: params.authType,
            authBump,
            padding,
            payload
        });

        // Strip prefix at offset 41 (1 Disc + 32 Seed + 1 Type + 1 Bump + 6 Pad)
        const data = this.stripPayloadPrefix(instruction.data, 41);

        const accounts = [
            meta(params.payer, "ws"),
            meta(params.wallet, "w"),
            meta(params.vault, "w"),
            meta(params.authority, "w"),
            meta("11111111111111111111111111111111" as Address, "r"), // SystemProgram
            meta("SysvarRent111111111111111111111111111111111" as Address, "r"), // Rent
            meta(params.config, "r"),
            meta(params.treasuryShard, "w"),
        ];

        return {
            programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
            accounts,
            data
        };
    }

    addAuthority(params: {
        payer: TransactionSigner;
        wallet: AddressLike;
        adminAuthority: AddressLike;
        newAuthority: AddressLike;
        config: AddressLike;
        treasuryShard: AddressLike;
        authType: number;
        newRole: number;
        authPubkey: ReadonlyUint8Array;
        credentialHash: ReadonlyUint8Array;
        authorizerSigner?: TransactionSigner;
    }) {
        const padding = new Uint8Array(6).fill(0);
        const payload = this.getAuthPayload(params.authType, params.authPubkey, params.credentialHash);

        const instruction = getAddAuthorityInstruction({
            payer: params.payer,
            wallet: resolveAddress(params.wallet),
            adminAuthority: resolveAddress(params.adminAuthority),
            newAuthority: resolveAddress(params.newAuthority),
            config: resolveAddress(params.config),
            treasuryShard: resolveAddress(params.treasuryShard),
            newType: params.authType,
            newRole: params.newRole,
            padding,
            payload
        });

        // Strip prefix at offset 9 (1 Disc + 1 Type + 1 Role + 6 Pad)
        const data = this.stripPayloadPrefix(instruction.data, 9);

        const accounts = [
            meta(params.payer, "ws"),
            meta(params.wallet, "r"),
            meta(params.adminAuthority, "w"), // Secp needs writable
            meta(params.newAuthority, "w"),
            meta("11111111111111111111111111111111" as Address, "r"), // System
        ];

        if (params.authorizerSigner) {
            accounts.push(meta(params.authorizerSigner, "s"));
        }

        accounts.push(meta(params.config, "r"));
        accounts.push(meta(params.treasuryShard, "w"));

        return {
            programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
            accounts,
            data
        };
    }

    removeAuthority(params: {
        payer: TransactionSigner;
        wallet: AddressLike;
        adminAuthority: AddressLike;
        targetAuthority: AddressLike;
        refundDestination: AddressLike;
        config: AddressLike;
        treasuryShard: AddressLike;
        authorizerSigner?: TransactionSigner;
    }) {
        const instruction = getRemoveAuthorityInstruction({
            payer: params.payer,
            wallet: resolveAddress(params.wallet),
            adminAuthority: resolveAddress(params.adminAuthority),
            targetAuthority: resolveAddress(params.targetAuthority),
            refundDestination: resolveAddress(params.refundDestination),
            config: resolveAddress(params.config),
            treasuryShard: resolveAddress(params.treasuryShard),
        });

        const accounts = [
            meta(params.payer, "ws"),
            meta(params.wallet, "r"),
            meta(params.adminAuthority, "w"), // Secp needs writable
            meta(params.targetAuthority, "w"), // To close it
            meta(params.refundDestination, "w"), // To receive rent
            meta("11111111111111111111111111111111" as Address, "r"), // System
        ];

        if (params.authorizerSigner) {
            accounts.push(meta(params.authorizerSigner, "s"));
        }

        accounts.push(meta(params.config, "r"));
        accounts.push(meta(params.treasuryShard, "w"));

        return {
            programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
            accounts,
            data: instruction.data
        };
    }

    transferOwnership(params: {
        payer: TransactionSigner;
        wallet: AddressLike;
        currentOwnerAuthority: AddressLike;
        newOwnerAuthority: AddressLike;
        config: AddressLike;
        treasuryShard: AddressLike;
        authType: number;
        authPubkey: ReadonlyUint8Array;
        credentialHash: ReadonlyUint8Array;
        authorizerSigner?: TransactionSigner;
    }) {
        const payload = this.getAuthPayload(params.authType, params.authPubkey, params.credentialHash);

        const instruction = getTransferOwnershipInstruction({
            payer: params.payer,
            wallet: resolveAddress(params.wallet),
            currentOwnerAuthority: resolveAddress(params.currentOwnerAuthority),
            newOwnerAuthority: resolveAddress(params.newOwnerAuthority),
            config: resolveAddress(params.config),
            treasuryShard: resolveAddress(params.treasuryShard),
            newType: params.authType,
            payload
        });

        // Strip prefix at offset 2 (1 Disc + 1 Type)
        const data = this.stripPayloadPrefix(instruction.data, 2);

        const accounts = [
            meta(params.payer, "ws"),
            meta(params.wallet, "r"),
            meta(params.currentOwnerAuthority, "w"), // Secp needs writable
            meta(params.newOwnerAuthority, "w"),
            meta("11111111111111111111111111111111" as Address, "r"),
            meta("SysvarRent111111111111111111111111111111111" as Address, "r"), // Rent
        ];

        if (params.authorizerSigner) {
            accounts.push(meta(params.authorizerSigner, "s"));
        }

        accounts.push(meta(params.config, "r"));
        accounts.push(meta(params.treasuryShard, "w"));

        return {
            programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
            accounts,
            data
        };
    }

    execute(params: {
        payer: TransactionSigner;
        wallet: AddressLike;
        authority: AddressLike;
        vault: AddressLike;
        config: AddressLike;
        treasuryShard: AddressLike;
        packedInstructions: Uint8Array;
        authorizerSigner?: TransactionSigner;
        sysvarInstructions?: AddressLike;
    }) {
        // Layout: [4, ...packedInstructions]
        const totalSize = 1 + params.packedInstructions.length;
        const data = new Uint8Array(totalSize);
        data[0] = 4;
        data.set(params.packedInstructions, 1);

        const finalAccounts = [
            meta(params.payer, "ws"),
            meta(params.wallet, "r"),
            meta(params.authority, "w"), // Secp needs writable
            meta(params.vault, "w"), // Vault is signer (role 4 in compact), but parsed as readonly in instruction accounts
            meta(params.config, "r"),
            meta(params.treasuryShard, "w"),
            meta("11111111111111111111111111111111" as Address, "r"), // System
        ];

        if (params.sysvarInstructions) {
            finalAccounts.push(meta(params.sysvarInstructions, "r"));
        }

        if (params.authorizerSigner) {
            finalAccounts.push(meta(params.authorizerSigner, "s"));
        }

        return {
            programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
            accounts: finalAccounts,
            data
        };
    }

    buildExecute(params: {
        payer: TransactionSigner;
        wallet: AddressLike;
        authority: AddressLike;
        vault: AddressLike;
        config: AddressLike;
        treasuryShard: AddressLike;
        innerInstructions: any[];
        authorizerSigner?: TransactionSigner;
        signature?: Uint8Array;
        sysvarInstructions?: AddressLike;
    }) {
        // Collect all unique accounts from inner instructions 
        const accountMap = new Map<string, number>();

        type AccInfo = { address: Address; role: AccountRole; signer?: TransactionSigner };

        // Define role constants from kit
        const READONLY_SIGNER = upgradeRoleToSigner(AccountRole.READONLY);
        const WRITABLE_SIGNER = upgradeRoleToSigner(AccountRole.WRITABLE);

        const vaultAddr = resolveAddress(params.vault);
        const walletAddr = resolveAddress(params.wallet);

        const allAccounts: AccInfo[] = [
            { address: params.payer.address, role: WRITABLE_SIGNER, signer: params.payer },
            { address: walletAddr, role: AccountRole.READONLY },
            { address: resolveAddress(params.authority), role: AccountRole.WRITABLE },
            { address: vaultAddr, role: AccountRole.READONLY },
            { address: resolveAddress(params.config), role: AccountRole.READONLY },
            { address: resolveAddress(params.treasuryShard), role: AccountRole.WRITABLE },
            { address: "11111111111111111111111111111111" as Address, role: AccountRole.READONLY },
        ];

        // Helper to check standard accounts
        const addAccount = (addr: Address, isSigner: boolean, isWritable: boolean, signerObj?: TransactionSigner) => {
            // Protect PDAs from being marked as signers
            if (addr === vaultAddr || addr === walletAddr) {
                isSigner = false;
            }

            if (!accountMap.has(addr)) {
                accountMap.set(addr, allAccounts.length);
                let role = isWritable ? AccountRole.WRITABLE : AccountRole.READONLY;
                if (isSigner) role = upgradeRoleToSigner(role);

                allAccounts.push({ address: addr, role, signer: signerObj });
            } else {
                // upgrade
                const idx = accountMap.get(addr)!;
                const current = allAccounts[idx];
                let role = current.role;

                // If current is readonly but new is writable, upgrade
                if (isWritable && (role === AccountRole.READONLY || role === READONLY_SIGNER)) {
                    role = (role === READONLY_SIGNER) ? WRITABLE_SIGNER : AccountRole.WRITABLE;
                }

                // If new is signer, upgrade (but PDAs are protected)
                if (isSigner && (role === AccountRole.READONLY || role === AccountRole.WRITABLE)) {
                    role = upgradeRoleToSigner(role);
                }

                allAccounts[idx].role = role;
                if (signerObj && !allAccounts[idx].signer) {
                    allAccounts[idx].signer = signerObj;
                }
            }
            return accountMap.get(addr)!;
        };

        // initialize map with standard accounts
        allAccounts.forEach((a, i) => accountMap.set(a.address, i));

        const compactIxs: CompactInstruction[] = [];

        for (const ix of params.innerInstructions) {
            const programIdIndex = addAccount(resolveAddress(ix.programAddress), false, false);

            const accountIndexes: number[] = [];
            for (const acc of (ix.accounts || [])) {
                // Handle various role input formats safely
                let isSigner = !!acc.isSigner;
                let isWritable = !!acc.isWritable;

                if (typeof acc.role === 'string') {
                    if (acc.role.includes('s')) isSigner = true;
                    if (acc.role.includes('w')) isWritable = true;
                } else if (typeof acc.role === 'number') {
                    if (acc.role === READONLY_SIGNER || acc.role === WRITABLE_SIGNER) isSigner = true;
                    if (acc.role === AccountRole.WRITABLE || acc.role === WRITABLE_SIGNER) isWritable = true;
                }

                const idx = addAccount(
                    resolveAddress(acc.address),
                    isSigner,
                    isWritable,
                );
                accountIndexes.push(idx);
            }

            compactIxs.push({
                programIdIndex,
                accountIndexes,
                data: ix.data instanceof Uint8Array ? ix.data : new Uint8Array(ix.data),
            });
        }

        const packed = packCompactInstructions(compactIxs);

        const sig = params.signature;
        const totalSize = 1 + packed.length + (sig?.length || 0);
        const data = new Uint8Array(totalSize);
        data[0] = 4; // Execute
        data.set(packed, 1);
        if (sig) data.set(sig, 1 + packed.length);

        if (params.sysvarInstructions) {
            addAccount(resolveAddress(params.sysvarInstructions), false, false);
        }

        if (params.authorizerSigner) {
            addAccount(params.authorizerSigner.address, true, false, params.authorizerSigner);
        }

        // Convert allAccounts to AccountMeta
        const accounts: (AccountMeta | AccountSignerMeta)[] = allAccounts.map(a => ({
            address: a.address,
            role: a.role,
            ...(a.signer ? { signer: a.signer } : {})
        }));

        return {
            programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
            accounts,
            data,
        };
    }

    createSession(params: {
        payer: TransactionSigner;
        wallet: AddressLike;
        adminAuthority: AddressLike;
        session: AddressLike;
        config: AddressLike;
        treasuryShard: AddressLike;
        sessionKey: ReadonlyUint8Array;
        expiresAt: bigint | number;
        authorizerSigner?: TransactionSigner;
    }) {
        const instruction = getCreateSessionInstruction({
            payer: params.payer,
            wallet: resolveAddress(params.wallet),
            adminAuthority: resolveAddress(params.adminAuthority),
            session: resolveAddress(params.session),
            config: resolveAddress(params.config),
            treasuryShard: resolveAddress(params.treasuryShard),
            sessionKey: params.sessionKey,
            expiresAt: BigInt(params.expiresAt)
        });

        const accounts = [
            meta(params.payer, "ws"),
            meta(params.wallet, "r"),
            meta(params.adminAuthority, "w"), // Secp needs writable
            meta(params.session, "w"),
            meta("11111111111111111111111111111111" as Address, "r"), // System
            meta("SysvarRent111111111111111111111111111111111" as Address, "r"), // Rent
        ];

        if (params.authorizerSigner) {
            accounts.push(meta(params.authorizerSigner, "s"));
        }

        accounts.push(meta(params.config, "r"));
        accounts.push(meta(params.treasuryShard, "w"));

        return {
            programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
            accounts,
            data: instruction.data
        };
    }

    async getWallet(address: AddressLike): Promise<WalletAccount> {
        const account = await fetchWalletAccount(this.rpc, resolveAddress(address));
        return account.data;
    }

    async getAuthority(address: AddressLike): Promise<AuthorityAccount> {
        const account = await fetchAuthorityAccount(this.rpc, resolveAddress(address));
        return account.data;
    }

    async getSession(address: AddressLike): Promise<SessionAccount> {
        const account = await fetchSessionAccount(this.rpc, resolveAddress(address));
        return account.data;
    }

    async getAuthorityByPublicKey(walletAddress: AddressLike, pubkey: Address | Uint8Array): Promise<AuthorityAccount | null> {
        const pubkeyBytes = typeof pubkey === 'string' ? Uint8Array.from(getAddressEncoder().encode(pubkey)) : pubkey;
        const [pda] = await findAuthorityPda(resolveAddress(walletAddress), pubkeyBytes);
        try {
            return await this.getAuthority(pda);
        } catch {
            return null;
        }
    }

    async getAuthorityByCredentialId(walletAddress: AddressLike, credentialIdHash: Uint8Array): Promise<AuthorityAccount | null> {
        const [pda] = await findAuthorityPda(resolveAddress(walletAddress), credentialIdHash);
        try {
            return await this.getAuthority(pda);
        } catch {
            return null;
        }
    }
}
