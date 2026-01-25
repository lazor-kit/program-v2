import {
    Address,
    address,
} from "@solana/addresses";
import {
    Rpc,
    SolanaRpcApi,
} from "@solana/rpc";
import {
    Transaction,
} from "@solana/transactions";
import {
    createTransactionMessage,
    setTransactionMessageFeePayer,
    setTransactionMessageLifetimeUsingBlockhash,
    appendTransactionMessageInstruction,
} from "@solana/transaction-messages";
import {
    TransactionSigner,
    addSignersToTransactionMessage,
    signTransactionMessageWithSigners,
} from "@solana/signers";
import {
    AccountMeta,
} from "@solana/instructions";
import {
    createWalletInstruction,
    createExecuteInstruction,
    addAuthorityInstruction,
    removeAuthorityInstruction,
    transferOwnershipInstruction,
    createSessionInstruction,
} from "./instructions";
import { createSecp256r1VerifyInstruction } from "./secp256r1";
import { findWalletPDA, findAuthorityPDA, findSessionPDA } from "./utils";
import { AuthType, Role } from "./types";
import { LAZORKIT_PROGRAM_ID } from "./constants";

export type LazorKitRpc = Rpc<SolanaRpcApi>;

export interface LazorKitClientConfig {
    rpc: LazorKitRpc;
    programId?: Address;
}

export class LazorKitClient {
    readonly rpc: LazorKitRpc;
    readonly programId: Address;

    constructor(config: LazorKitClientConfig) {
        this.rpc = config.rpc;
        this.programId = config.programId || LAZORKIT_PROGRAM_ID;
    }

    // ===================================
    // Transaction Builders
    // ===================================

    /**
     * Creates a transaction to initialize a new wallet.
     */
    async createWallet(params: {
        payer: TransactionSigner;
        userSeed: Uint8Array;
        authType: AuthType;
        authPubkey: Uint8Array;
        credentialHash: Uint8Array;
    }): Promise<Transaction> {
        const walletIx = await createWalletInstruction(
            params.payer.address,
            params.userSeed,
            params.authType,
            params.authPubkey,
            params.credentialHash,
            this.programId
        );

        return this.buildTransaction(params.payer, [walletIx]);
    }

    /**
     * Creates a transaction to execute a batch of instructions.
     */
    async execute(params: {
        payer: TransactionSigner;
        wallet: Address;
        authority: TransactionSigner; // The authority signing the execution (or session key)
        instructions: Uint8Array; // Compact instructions
        remainingAccounts?: AccountMeta[];
        isSecp256r1?: boolean;
        isSession?: boolean; // If true, authority is treated as a session key
        // Optional Secp256r1 specific args
        secpPreverify?: {
            message: Uint8Array;
            pubkey: Uint8Array;
            signature: Uint8Array;
        };
    }): Promise<Transaction> {
        const ixs = [];

        // 1. Optional Secp256r1 Pre-verify
        if (params.secpPreverify) {
            ixs.push(
                createSecp256r1VerifyInstruction(
                    params.secpPreverify.message,
                    params.secpPreverify.pubkey,
                    params.secpPreverify.signature
                )
            );
        }

        let authorityPda: Address;
        let authoritySigner = params.authority.address;

        if (params.isSecp256r1) {
            throw new Error("Secp256r1 execution requires explicit authority PDA handling.");
        } else {
            // Ed25519: Seed is the pubkey
            const seed = await this.getAddressBytes(params.authority.address);

            if (params.isSession) {
                const [pda] = await findSessionPDA(params.wallet, seed, this.programId);
                authorityPda = pda;
            } else {
                const [pda] = await findAuthorityPDA(params.wallet, seed, this.programId);
                authorityPda = pda;
            }
        }

        const executeIx = await createExecuteInstruction(
            params.payer.address,
            params.wallet,
            authorityPda,
            params.instructions,
            params.remainingAccounts || [],
            authoritySigner,
            this.programId,
            params.isSecp256r1
        );
        ixs.push(executeIx);

        return this.buildTransaction(params.payer, ixs, [params.authority]);
    }

    async addAuthority(params: {
        payer: TransactionSigner;
        wallet: Address;
        adminAuthority: TransactionSigner; // The admin approving this
        newAuthType: AuthType;
        newAuthRole: Role;
        newPubkey: Uint8Array;
        newHash: Uint8Array;
        // Secp specific
        secpPreverify?: {
            message: Uint8Array;
            pubkey: Uint8Array;
            signature: Uint8Array;
        };
    }): Promise<Transaction> {
        const ixs = [];
        if (params.secpPreverify) {
            ixs.push(createSecp256r1VerifyInstruction(
                params.secpPreverify.message,
                params.secpPreverify.pubkey,
                params.secpPreverify.signature
            ));
        }

        // Resolve Admin PDA (Assume Ed25519 for signer)
        const seed = await this.getAddressBytes(params.adminAuthority.address);
        const [adminPda] = await findAuthorityPDA(params.wallet, seed, this.programId);

        const addIx = await addAuthorityInstruction(
            params.payer.address,
            params.wallet,
            adminPda,
            params.newAuthType,
            params.newPubkey,
            params.newHash,
            params.newAuthRole,
            params.adminAuthority.address,
            new Uint8Array(0),
            this.programId
        );
        ixs.push(addIx);

        return this.buildTransaction(params.payer, ixs, [params.adminAuthority]);
    }

    async removeAuthority(params: {
        payer: TransactionSigner;
        wallet: Address;
        adminAuthority: TransactionSigner; // Must be an Admin
        targetAuthority: Address;
        refundDestination: Address;
        secpPreverify?: {
            message: Uint8Array;
            pubkey: Uint8Array;
            signature: Uint8Array;
        };
    }): Promise<Transaction> {
        const ixs = [];
        if (params.secpPreverify) {
            ixs.push(createSecp256r1VerifyInstruction(
                params.secpPreverify.message,
                params.secpPreverify.pubkey,
                params.secpPreverify.signature
            ));
        }

        // Resolve Admin PDA (Assume Ed25519 for signer)
        const seed = await this.getAddressBytes(params.adminAuthority.address);
        const [adminPda] = await findAuthorityPDA(params.wallet, seed, this.programId);

        // Resolve Target PDA (Assume Ed25519 for now, or pass in type/seed?)
        // Wait, removeAuthorityInstruction takes targetAuthorityPda directly.
        // The user should probably pass the PDA address if they know it, or the public key and we derive it.
        // For simplicity, let's assume 'targetAuthority' passed in IS the PDA address to remove.
        // If not, we'd need more info (AuthType + Pubkey/Hash) to derive it.
        // Let's assume it IS the PDA for now as that's what the instruction expects.
        // But for consistency with addAuthority, maybe we should accept the pubkey and type?
        // No, removal targets a specific PDA. If we just have the public key, we can derive it if we know the type.
        // Let's stick to passing the PDA address for now to be precise.

        const removeIx = await removeAuthorityInstruction(
            params.payer.address,
            params.wallet,
            adminPda,
            params.targetAuthority, // The PDA of the authority to remove
            params.refundDestination,
            params.adminAuthority.address,
            new Uint8Array(0),
            this.programId
        );
        ixs.push(removeIx);

        return this.buildTransaction(params.payer, ixs, [params.adminAuthority]);
    }

    async transferOwnership(params: {
        payer: TransactionSigner;
        wallet: Address;
        currentOwner: TransactionSigner; // Must be Owner
        newType: AuthType;
        newPubkey: Uint8Array;
        newHash: Uint8Array;
        secpPreverify?: {
            message: Uint8Array;
            pubkey: Uint8Array;
            signature: Uint8Array;
        };
    }): Promise<Transaction> {
        const ixs = [];
        if (params.secpPreverify) {
            ixs.push(createSecp256r1VerifyInstruction(
                params.secpPreverify.message,
                params.secpPreverify.pubkey,
                params.secpPreverify.signature
            ));
        }

        // Resolve Current Owner PDA
        const seed = await this.getAddressBytes(params.currentOwner.address);
        const [ownerPda] = await findAuthorityPDA(params.wallet, seed, this.programId);

        const transferIx = await transferOwnershipInstruction(
            params.payer.address,
            params.wallet,
            ownerPda,
            params.newType,
            params.newPubkey,
            params.newHash,
            params.currentOwner.address,
            new Uint8Array(0),
            this.programId
        );
        ixs.push(transferIx);

        return this.buildTransaction(params.payer, ixs, [params.currentOwner]);
    }

    async createSession(params: {
        payer: TransactionSigner;
        wallet: Address;
        authorizer: TransactionSigner; // Admin or Owner
        sessionKey: Uint8Array;
        expiresAt: bigint;
        secpPreverify?: {
            message: Uint8Array;
            pubkey: Uint8Array;
            signature: Uint8Array;
        };
    }): Promise<Transaction> {
        const ixs = [];
        if (params.secpPreverify) {
            ixs.push(createSecp256r1VerifyInstruction(
                params.secpPreverify.message,
                params.secpPreverify.pubkey,
                params.secpPreverify.signature
            ));
        }

        // Resolve Authorizer PDA
        const seed = await this.getAddressBytes(params.authorizer.address);
        const [authPda] = await findAuthorityPDA(params.wallet, seed, this.programId);

        const sessionIx = await createSessionInstruction(
            params.payer.address,
            params.wallet,
            authPda,
            params.sessionKey,
            params.expiresAt,
            params.authorizer.address,
            new Uint8Array(0),
            this.programId
        );
        ixs.push(sessionIx);

        return this.buildTransaction(params.payer, ixs, [params.authorizer]);
    }


    // ===================================
    // Helpers
    // ===================================

    private async buildTransaction(
        payer: TransactionSigner,
        instructions: any[], // Instruction[]
        otherSigners: TransactionSigner[] = []
    ): Promise<Transaction> {
        const { value: blockhash } = await this.rpc.getLatestBlockhash().send();

        const message = createTransactionMessage({ version: 0 });
        const messageWithFeePayer = setTransactionMessageFeePayer(
            payer.address,
            message
        );
        const messageWithLifetime = setTransactionMessageLifetimeUsingBlockhash(
            blockhash,
            messageWithFeePayer
        );

        // Append instructions
        const messageWithIxs = instructions.reduce((msg, ix) => {
            return appendTransactionMessageInstruction(ix, msg);
        }, messageWithLifetime);

        // Add Signers
        const allSigners = [payer, ...otherSigners];
        const uniqueSigners = allSigners.filter((s, i, self) =>
            self.findIndex(o => o.address === s.address) === i
        );

        // Create message with signers attached
        const messageWithSigners = addSignersToTransactionMessage(uniqueSigners, messageWithIxs);

        // Sign transaction
        const signedTx = await signTransactionMessageWithSigners(messageWithSigners);

        return signedTx;
    }

    private async getAddressBytes(addr: Address): Promise<Uint8Array> {
        const { getAddressEncoder } = await import("@solana/addresses");
        return getAddressEncoder().encode(addr) as Uint8Array;
    }
}
