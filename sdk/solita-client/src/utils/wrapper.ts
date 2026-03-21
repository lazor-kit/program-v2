import {
    Connection,
    Keypair,
    PublicKey,
    Transaction,
    TransactionInstruction,
    SystemProgram,
    type AccountMeta
} from "@solana/web3.js";

import { LazorWeb3Client } from "./client";
import {
    findWalletPda,
    findVaultPda,
    findAuthorityPda,
    findConfigPda,
    findTreasuryShardPda,
    findSessionPda,
    PROGRAM_ID
} from "./pdas";

import { Role } from "../generated";
import { type Secp256r1Signer, buildSecp256r1Message, buildSecp256r1PrecompileIx, appendSecp256r1Sysvars, buildAuthPayload, buildAuthenticatorData, readCurrentSlot } from "./secp256r1";
import { computeAccountsHash, packCompactInstructions, type CompactInstruction } from "./packing";
import bs58 from "bs58";

// ─── Enums ───────────────────────────────────────────────────────────────────

export enum AuthType {
    Ed25519 = 0,
    Secp256r1 = 1
}

export { Role };

// ─── Constants ───────────────────────────────────────────────────────────────

export const AUTHORITY_ACCOUNT_HEADER_SIZE = 48;
export const AUTHORITY_ACCOUNT_ED25519_SIZE = 48 + 32;   // 80 bytes
export const AUTHORITY_ACCOUNT_SECP256R1_SIZE = 48 + 65; // 113 bytes
export const DISCRIMINATOR_AUTHORITY = 2;


// ─── Types ───────────────────────────────────────────────────────────────────

/**
 * Parameters for creating a new LazorKit wallet.
 *
 * - `userSeed` is optional — the SDK auto-generates a random 32-byte seed if omitted.
 * - For Ed25519: only `owner` public key is required.
 * - For Secp256r1: `pubkey` (33-byte compressed P-256 key) and `credentialHash` (32-byte SHA-256 of credential ID) are required.
 */
export type CreateWalletParams =
    | {
        payer: Keypair;
        authType: AuthType.Ed25519;
        owner: PublicKey;
        userSeed?: Uint8Array;
    }
    | {
        payer: Keypair;
        authType?: AuthType.Secp256r1;
        pubkey: Uint8Array;       // 33-byte compressed P-256 public key
        credentialHash: Uint8Array; // 32-byte SHA-256 hash of the WebAuthn credential ID
        userSeed?: Uint8Array;
    };

/**
 * Identifies the admin/owner who is authorizing a privileged action.
 *
 * - Ed25519: provide `adminSigner` Keypair — SDK derives the Authority PDA automatically.
 * - Secp256r1: provide `adminCredentialHash` to derive the Authority PDA.
 *   The actual signature verification is done via a preceding Secp256r1 precompile instruction.
 */
export type AdminSignerOptions =
    | {
        adminType: AuthType.Ed25519;
        adminSigner: Keypair;
    }
    | {
        adminType?: AuthType.Secp256r1;
        /**
         * 32-byte SHA-256 hash of the WebAuthn credential ID.
         * Used to derive the admin Authority PDA.
         * The actual Secp256r1 signature must be provided as a separate
         * precompile instruction prepended to the transaction.
         */
        adminCredentialHash: Uint8Array;
    };

// ─── LazorClient ─────────────────────────────────────────────────────────────

/**
 * High-level client for the LazorKit Smart Wallet program.
 *
 * Two usage layers:
 *
 * **Layer 1 — Instruction builders** (`createWallet`, `addAuthority`, `execute`, …)
 *   Return a `TransactionInstruction` (plus any derived addresses).
 *   Callers compose multiple instructions and send the transaction themselves.
 *
 * **Layer 2 — Transaction builders** (`createWalletTxn`, `addAuthorityTxn`, …)
 *   Wrap Layer-1 results in a `Transaction` object ready to be signed and sent.
 *
 * Sending is always the caller's responsibility — use `connection.sendRawTransaction` or
 * any helper you prefer so that this SDK stays free of transport assumptions.
 */
export class LazorClient {
    public client: LazorWeb3Client;

    constructor(
        public connection: Connection,
        public programId: PublicKey = PROGRAM_ID
    ) {
        this.client = new LazorWeb3Client(programId);
    }

    // ─── PDA helpers (instance convenience) ──────────────────────────────────

    /** Derives the Wallet PDA from a 32-byte seed. */
    getWalletPda(userSeed: Uint8Array): PublicKey {
        return findWalletPda(userSeed, this.programId)[0];
    }

    /** Derives the Vault PDA from a Wallet PDA. */
    getVaultPda(walletPda: PublicKey): PublicKey {
        return findVaultPda(walletPda, this.programId)[0];
    }

    /**
     * Derives an Authority PDA.
     * @param idSeed For Ed25519: 32-byte public key. For Secp256r1: 32-byte credential hash.
     */
    getAuthorityPda(walletPda: PublicKey, idSeed: Uint8Array | PublicKey): PublicKey {
        const seed = idSeed instanceof PublicKey ? idSeed.toBytes() : idSeed;
        return findAuthorityPda(walletPda, seed, this.programId)[0];
    }

    /** Derives a Session PDA from a wallet PDA and session public key. */
    getSessionPda(walletPda: PublicKey, sessionKey: PublicKey): PublicKey {
        return findSessionPda(walletPda, sessionKey, this.programId)[0];
    }

    /** Derives the global Config PDA. */
    getConfigPda(): PublicKey {
        return findConfigPda(this.programId)[0];
    }

    /** Derives a Treasury Shard PDA for a given shard index. */
    getTreasuryShardPda(shardId: number): PublicKey {
        return findTreasuryShardPda(shardId, this.programId)[0];
    }

    // ─── Internal helpers ─────────────────────────────────────────────────────

    private getShardId(pubkey: PublicKey): number {
        return pubkey.toBytes().reduce((a, b) => a + b, 0) % 16;
    }

    private getCommonPdas(payerPubkey: PublicKey): { configPda: PublicKey; treasuryShard: PublicKey } {
        const configPda = findConfigPda(this.programId)[0];
        const shardId = this.getShardId(payerPubkey);
        const treasuryShard = findTreasuryShardPda(shardId, this.programId)[0];
        return { configPda, treasuryShard };
    }

    // ─── Layer 1: Instruction builders ───────────────────────────────────────

    /**
     * Builds a `CreateWallet` instruction.
     *
     * - `userSeed` is optional — a random 32-byte seed is generated when omitted.
     * - Returns the derived `walletPda`, `authorityPda`, and the actual `userSeed` used,
     *   so callers can store the seed for later recovery.
     */
    async createWallet(params: CreateWalletParams): Promise<{
        ix: TransactionInstruction;
        walletPda: PublicKey;
        authorityPda: PublicKey;
        userSeed: Uint8Array;
    }> {
        const userSeed = params.userSeed ?? crypto.getRandomValues(new Uint8Array(32));
        const [walletPda] = findWalletPda(userSeed, this.programId);
        const [vaultPda] = findVaultPda(walletPda, this.programId);

        const authType = params.authType ?? AuthType.Secp256r1;
        let authorityPda: PublicKey;
        let authBump: number;
        let authPubkey: Uint8Array;
        let credentialHash: Uint8Array = new Uint8Array(32);

        if (params.authType === AuthType.Ed25519) {
            authPubkey = params.owner.toBytes();
            [authorityPda, authBump] = findAuthorityPda(walletPda, authPubkey, this.programId);
        } else {
            const p = params as { pubkey: Uint8Array; credentialHash: Uint8Array };
            authPubkey = p.pubkey;
            credentialHash = p.credentialHash;
            [authorityPda, authBump] = findAuthorityPda(walletPda, credentialHash, this.programId);
        }

        const { configPda, treasuryShard } = this.getCommonPdas(params.payer.publicKey);

        const ix = this.client.createWallet({
            config: configPda,
            treasuryShard,
            payer: params.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: authorityPda,
            userSeed,
            authType,
            authBump,
            authPubkey,
            credentialHash,
        });

        return { ix, walletPda, authorityPda, userSeed };
    }

    /**
     * Builds an `AddAuthority` instruction.
     *
     * - `role` defaults to `Role.Spender`.
     * - `authType` defaults to `AuthType.Secp256r1`.
     * - `adminAuthorityPda` can be provided to override auto-derivation.
     */
    async addAuthority(params: {
        payer: Keypair;
        walletPda: PublicKey;
        newAuthorityPubkey: Uint8Array;
        authType?: AuthType;
        role?: Role;
        credentialHash?: Uint8Array;
        /** Override the admin Authority PDA instead of auto-deriving it. */
        adminAuthorityPda?: PublicKey;
    } & AdminSignerOptions): Promise<{ ix: TransactionInstruction; newAuthority: PublicKey }> {
        const { configPda, treasuryShard } = this.getCommonPdas(params.payer.publicKey);

        const authType = params.authType ?? AuthType.Secp256r1;
        const role = params.role ?? Role.Spender;
        const adminType = params.adminType ?? AuthType.Secp256r1;

        const idSeed = authType === AuthType.Secp256r1
            ? (params.credentialHash ?? new Uint8Array(32))
            : params.newAuthorityPubkey.slice(0, 32);
        const [newAuthority] = findAuthorityPda(params.walletPda, idSeed, this.programId);

        let adminAuthority: PublicKey;
        if (params.adminAuthorityPda) {
            adminAuthority = params.adminAuthorityPda;
        } else if (adminType === AuthType.Ed25519) {
            const p = params as { adminSigner: Keypair };
            [adminAuthority] = findAuthorityPda(params.walletPda, p.adminSigner.publicKey.toBytes(), this.programId);
        } else {
            const p = params as { adminCredentialHash: Uint8Array };
            [adminAuthority] = findAuthorityPda(params.walletPda, p.adminCredentialHash, this.programId);
        }

        const ix = this.client.addAuthority({
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            adminAuthority,
            newAuthority,
            config: configPda,
            treasuryShard,
            authType,
            newRole: role,
            authPubkey: params.newAuthorityPubkey,
            credentialHash: params.credentialHash,
            authorizerSigner: adminType === AuthType.Ed25519 ? (params as any).adminSigner.publicKey : undefined,
        });

        return { ix, newAuthority };
    }

    /**
     * Builds a `RemoveAuthority` instruction.
     *
     * - `refundDestination` is optional — defaults to `payer.publicKey`.
     */
    async removeAuthority(params: {
        payer: Keypair;
        walletPda: PublicKey;
        authorityToRemovePda: PublicKey;
        /** Where to send the recovered rent SOL. Defaults to payer. */
        refundDestination?: PublicKey;
    } & AdminSignerOptions): Promise<TransactionInstruction> {
        const { configPda, treasuryShard } = this.getCommonPdas(params.payer.publicKey);
        const refundDestination = params.refundDestination ?? params.payer.publicKey;

        let adminAuthority: PublicKey;
        if (params.adminType === AuthType.Ed25519) {
            [adminAuthority] = findAuthorityPda(params.walletPda, params.adminSigner.publicKey.toBytes(), this.programId);
        } else {
            [adminAuthority] = findAuthorityPda(params.walletPda, params.adminCredentialHash, this.programId);
        }

        return this.client.removeAuthority({
            config: configPda,
            treasuryShard,
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            adminAuthority,
            targetAuthority: params.authorityToRemovePda,
            refundDestination,
            authorizerSigner: params.adminType === AuthType.Ed25519 ? params.adminSigner.publicKey : undefined,
        });
    }

    /**
     * Builds a `CreateSession` instruction.
     *
     * - `sessionKey` is optional — an ephemeral Keypair is auto-generated when omitted.
     * - `expiresAt` is optional — defaults to 1 hour from now (in Unix seconds).
     * - Returns the generated `sessionKeypair` so the caller can store / use it.
     */
    async createSession(params: {
        payer: Keypair;
        walletPda: PublicKey;
        /** Session public key. Omit to let the SDK auto-generate an ephemeral keypair. */
        sessionKey?: PublicKey;
        /**
         * Absolute slot height (or Unix timestamp) at which the session expires.
         * Defaults to `Date.now() / 1000 + 3600` (1 hour from now).
         */
        expiresAt?: bigint | number;
    } & AdminSignerOptions): Promise<{
        ix: TransactionInstruction;
        sessionPda: PublicKey;
        /** The session keypair — only set when auto-generated; null if caller supplied sessionKey. */
        sessionKeypair: Keypair | null;
    }> {
        const { configPda, treasuryShard } = this.getCommonPdas(params.payer.publicKey);
        const expiresAt = params.expiresAt != null
            ? BigInt(params.expiresAt)
            : BigInt(Math.floor(Date.now() / 1000) + 3600);
        const adminType = params.adminType ?? AuthType.Secp256r1;

        let sessionKeypair: Keypair | null = null;
        let sessionKeyPubkey: PublicKey;
        if (params.sessionKey) {
            sessionKeyPubkey = params.sessionKey;
        } else {
            sessionKeypair = Keypair.generate();
            sessionKeyPubkey = sessionKeypair.publicKey;
        }

        const [sessionPda] = findSessionPda(params.walletPda, sessionKeyPubkey, this.programId);

        let adminAuthority: PublicKey;
        if (adminType === AuthType.Ed25519) {
            const p = params as { adminSigner: Keypair };
            [adminAuthority] = findAuthorityPda(params.walletPda, p.adminSigner.publicKey.toBytes(), this.programId);
        } else {
            const p = params as { adminCredentialHash: Uint8Array };
            [adminAuthority] = findAuthorityPda(params.walletPda, p.adminCredentialHash, this.programId);
        }

        const ix = this.client.createSession({
            config: configPda,
            treasuryShard,
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            adminAuthority,
            session: sessionPda,
            sessionKey: Array.from(sessionKeyPubkey.toBytes()),
            expiresAt,
            authorizerSigner: adminType === AuthType.Ed25519 ? (params as any).adminSigner.publicKey : undefined,
        });

        return { ix, sessionPda, sessionKeypair };
    }

    /**
     * Builds a `CloseSession` instruction.
     */
    async closeSession(params: {
        payer: Keypair;
        walletPda: PublicKey;
        sessionPda: PublicKey;
        /** Override the Config PDA if needed. */
        configPda?: PublicKey;
        authorizer?: {
            authorizerPda: PublicKey;
            signer: Keypair;
        };
    }): Promise<TransactionInstruction> {
        const configPda = params.configPda ?? findConfigPda(this.programId)[0];

        return this.client.closeSession({
            config: configPda,
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            session: params.sessionPda,
            authorizer: params.authorizer?.authorizerPda,
            authorizerSigner: params.authorizer?.signer.publicKey,
        });
    }

    /**
     * Builds an `Execute` instruction using the high-level builder that handles
     * account deduplication and CompactInstruction packing automatically.
     *
     * - `authorityPda` is optional when `signer` (Ed25519 Keypair) is provided —
     *   the SDK derives the Authority PDA from `signer.publicKey`.
     * - `vaultPda` is optional — derived from `walletPda` when omitted.
     */
    async execute(params: {
        payer: Keypair;
        walletPda: PublicKey;
        innerInstructions: TransactionInstruction[];
        /** Authority PDA. Omit when `signer` is an Ed25519 Keypair — SDK auto-derives it. */
        authorityPda?: PublicKey;
        /** Ed25519 keypair that signs the transaction (authorizer signer). */
        signer?: Keypair;
        /** Secp256r1 signature bytes appended to instruction data. */
        signature?: Uint8Array;
        /** Override vault PDA if different from the canonical derivation. */
        vaultPda?: PublicKey;
    }): Promise<TransactionInstruction> {
        const { configPda, treasuryShard } = this.getCommonPdas(params.payer.publicKey);
        const vaultPda = params.vaultPda ?? findVaultPda(params.walletPda, this.programId)[0];

        // Auto-derive authorityPda from signer if not provided
        let authorityPda = params.authorityPda;
        if (!authorityPda && params.signer) {
            [authorityPda] = findAuthorityPda(params.walletPda, params.signer.publicKey.toBytes(), this.programId);
        }
        if (!authorityPda) {
            throw new Error(
                "execute(): either `authorityPda` or `signer` must be provided so the SDK can identify the Authority PDA."
            );
        }

        const ix = this.client.buildExecute({
            config: configPda,
            treasuryShard,
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            authority: authorityPda,
            vault: vaultPda,
            innerInstructions: params.innerInstructions,
            authorizerSigner: params.signer?.publicKey,
        });

        if (params.signature) {
            const newData = Buffer.alloc(ix.data.length + params.signature.length);
            ix.data.copy(newData);
            newData.set(params.signature, ix.data.length);
            ix.data = newData;
        }

        return ix;
    }

    /**
     * Builds a bundle of two instructions for Secp256r1 Execution:
     * 1. Secp256r1 Precompile Instruction
     * 2. LazorKit Execute Instruction with appended signature payload
     * 
     * @returns Array of [PrecompileIx, ExecuteIx]
     */
    async executeWithSecp256r1(params: {
        payer: Keypair;
        walletPda: PublicKey;
        innerInstructions: TransactionInstruction[];
        signer: Secp256r1Signer;
        /** Optional: absolute slot height for liveness proof. fetched automatically if 0n/omitted */
        slot?: bigint;
        rpId?: string;
    }): Promise<{ precompileIx: TransactionInstruction, executeIx: TransactionInstruction }> {
        const { configPda, treasuryShard } = this.getCommonPdas(params.payer.publicKey);
        const vaultPda = findVaultPda(params.walletPda, this.programId)[0];
        const [authorityPda] = findAuthorityPda(params.walletPda, params.signer.credentialIdHash, this.programId);

        // 1. Replicate deduplication to get exact account list for hashing
        const baseAccounts: PublicKey[] = [
            params.payer.publicKey,
            params.walletPda,
            authorityPda,
            vaultPda,
            configPda,
            treasuryShard,
            SystemProgram.programId,
        ];

        const accountMap = new Map<string, number>();
        const accountMetas: AccountMeta[] = [];

        baseAccounts.forEach((pk, idx) => {
            accountMap.set(pk.toBase58(), idx);
            accountMetas.push({
                pubkey: pk,
                isWritable: idx === 0 || idx === 2 || idx === 3 || idx === 5,
                isSigner: idx === 0,
            });
        });

        const vaultKey = vaultPda.toBase58();
        const walletKey = params.walletPda.toBase58();

        const addAccount = (pubkey: PublicKey, isSigner: boolean, isWritable: boolean): number => {
            const key = pubkey.toBase58();
            if (key === vaultKey || key === walletKey) isSigner = false;

            if (!accountMap.has(key)) {
                const idx = accountMetas.length;
                accountMap.set(key, idx);
                accountMetas.push({ pubkey, isWritable, isSigner });
                return idx;
            } else {
                const idx = accountMap.get(key)!;
                const existing = accountMetas[idx];
                if (isWritable) existing.isWritable = true;
                if (isSigner) existing.isSigner = true;
                return idx;
            }
        };

        const compactIxs: CompactInstruction[] = [];
        for (const ix of params.innerInstructions) {
            const programIdIndex = addAccount(ix.programId, false, false);
            const accountIndexes: number[] = [];
            for (const acc of ix.keys) {
                accountIndexes.push(addAccount(acc.pubkey, acc.isSigner, acc.isWritable));
            }
            compactIxs.push({ programIdIndex, accountIndexes, data: ix.data });
        }

        const packedInstructions = packCompactInstructions(compactIxs);

        // 2. Load Slot
        const slot = params.slot ?? await readCurrentSlot(this.connection);

        // 3. Build Auth Payload
        const executeIxBase = new TransactionInstruction({
            programId: this.programId,
            keys: [...accountMetas], // copy
            data: Buffer.alloc(0)
        });
        const { ix: ixWithSysvars, sysvarIxIndex, sysvarSlotIndex } = appendSecp256r1Sysvars(executeIxBase);

        const authenticatorData = await buildAuthenticatorData(params.rpId);

        const authPayload = buildAuthPayload({
            sysvarIxIndex,
            sysvarSlotIndex,
            authenticatorData,
            slot,
            rpId: params.rpId
        });

        // 4. Compute Accounts Hash
        const accountsHash = await computeAccountsHash(ixWithSysvars.keys, compactIxs);

        // 5. Build Combined Signed Payload = PackedInstructions + AccountsHash
        const signedPayload = new Uint8Array(packedInstructions.length + 32);
        signedPayload.set(packedInstructions, 0);
        signedPayload.set(accountsHash, packedInstructions.length);

        // 6. Generate Message to Sign
        const message = await buildSecp256r1Message({
            discriminator: 4, // Execute
            authPayload,
            signedPayload,
            payer: params.payer.publicKey,
            programId: this.programId,
            slot
        });

        // 7. Get Precompile Instruction
        const precompileIx = await buildSecp256r1PrecompileIx(params.signer, message);

        // 8. Build final Execute Instruction Data
        // Layout: [disc(4)] [packedInstructions] [authPayload]
        const finalData = Buffer.alloc(1 + packedInstructions.length + authPayload.length);
        finalData[0] = 4; // disc
        finalData.set(packedInstructions, 1);
        finalData.set(authPayload, 1 + packedInstructions.length);

        const executeIx = new TransactionInstruction({
            programId: this.programId,
            keys: ixWithSysvars.keys,
            data: finalData
        });

        return { precompileIx, executeIx };
    }


    /**
     * Builds a `CloseWallet` instruction.
     *
     * - `destination` is optional — defaults to `payer.publicKey`.
     * - `vaultPda` is optional — derived from `walletPda` when omitted.
     * - `adminAuthorityPda` is optional — auto-derived from admin signer credentials.
     */
    async closeWallet(params: {
        payer: Keypair;
        walletPda: PublicKey;
        /** Where to sweep all remaining SOL. Defaults to payer. */
        destination?: PublicKey;
        /** Override the Vault PDA if needed. */
        vaultPda?: PublicKey;
        /** Override the owner Authority PDA instead of auto-deriving it. */
        adminAuthorityPda?: PublicKey;
    } & AdminSignerOptions): Promise<TransactionInstruction> {
        const vaultPda = params.vaultPda ?? findVaultPda(params.walletPda, this.programId)[0];
        const destination = params.destination ?? params.payer.publicKey;

        let ownerAuthority: PublicKey;
        if (params.adminAuthorityPda) {
            ownerAuthority = params.adminAuthorityPda;
        } else if (params.adminType === AuthType.Ed25519) {
            [ownerAuthority] = findAuthorityPda(params.walletPda, params.adminSigner.publicKey.toBytes(), this.programId);
        } else {
            [ownerAuthority] = findAuthorityPda(params.walletPda, params.adminCredentialHash ?? new Uint8Array(), this.programId);
        }

        const ix = this.client.closeWallet({
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            vault: vaultPda,
            ownerAuthority,
            destination,
            ownerSigner: params.adminType === AuthType.Ed25519 ? params.adminSigner.publicKey : undefined,
        });

        // Required by on-chain close logic
        ix.keys.push({
            pubkey: SystemProgram.programId,
            isWritable: false,
            isSigner: false,
        });

        return ix;
    }

    /**
     * Builds a `TransferOwnership` instruction.
     */
    async transferOwnership(params: {
        payer: Keypair;
        walletPda: PublicKey;
        currentOwnerAuthority: PublicKey;
        newOwnerAuthority: PublicKey;
        authType: AuthType;
        authPubkey: Uint8Array;
        credentialHash?: Uint8Array;
        /** Ed25519 signer (optional — for Secp256r1, auth comes via precompile instruction). */
        signer?: Keypair;
    }): Promise<TransactionInstruction> {
        const { configPda, treasuryShard } = this.getCommonPdas(params.payer.publicKey);

        return this.client.transferOwnership({
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            currentOwnerAuthority: params.currentOwnerAuthority,
            newOwnerAuthority: params.newOwnerAuthority,
            config: configPda,
            treasuryShard,
            authType: params.authType,
            authPubkey: params.authPubkey,
            credentialHash: params.credentialHash,
            authorizerSigner: params.signer?.publicKey,
        });
    }

    // ─── Admin instructions ───────────────────────────────────────────────────

    /**
     * Builds an `InitializeConfig` instruction.
     */
    async initializeConfig(params: {
        admin: Keypair;
        walletFee: bigint | number;
        actionFee: bigint | number;
        numShards: number;
    }): Promise<TransactionInstruction> {
        const configPda = findConfigPda(this.programId)[0];
        return this.client.initializeConfig({
            admin: params.admin.publicKey,
            config: configPda,
            walletFee: BigInt(params.walletFee),
            actionFee: BigInt(params.actionFee),
            numShards: params.numShards,
        });
    }

    /**
     * Builds an `InitTreasuryShard` instruction.
     */
    async initTreasuryShard(params: {
        payer: Keypair;
        shardId: number;
    }): Promise<TransactionInstruction> {
        const configPda = findConfigPda(this.programId)[0];
        const [treasuryShard] = findTreasuryShardPda(params.shardId, this.programId);
        return this.client.initTreasuryShard({
            payer: params.payer.publicKey,
            config: configPda,
            treasuryShard,
            shardId: params.shardId,
        });
    }

    /**
     * Builds a `SweepTreasury` instruction.
     */
    async sweepTreasury(params: {
        admin: Keypair;
        shardId: number;
        destination: PublicKey;
    }): Promise<TransactionInstruction> {
        const configPda = findConfigPda(this.programId)[0];
        const [treasuryShard] = findTreasuryShardPda(params.shardId, this.programId);
        return this.client.sweepTreasury({
            admin: params.admin.publicKey,
            config: configPda,
            treasuryShard,
            destination: params.destination,
            shardId: params.shardId,
        });
    }

    // ─── Layer 2: Transaction builders ───────────────────────────────────────
    // Return a `Transaction` object with `feePayer` set. Signing and sending
    // is always the caller's responsibility.

    async createWalletTxn(params: Parameters<typeof this.createWallet>[0]): Promise<{
        transaction: Transaction;
        walletPda: PublicKey;
        authorityPda: PublicKey;
        userSeed: Uint8Array;
    }> {
        const { ix, walletPda, authorityPda, userSeed } = await this.createWallet(params);
        const transaction = new Transaction().add(ix);
        transaction.feePayer = params.payer.publicKey;
        return { transaction, walletPda, authorityPda, userSeed };
    }

    async addAuthorityTxn(params: Parameters<typeof this.addAuthority>[0]): Promise<{
        transaction: Transaction;
        newAuthority: PublicKey;
    }> {
        const { ix, newAuthority } = await this.addAuthority(params);
        const transaction = new Transaction().add(ix);
        transaction.feePayer = params.payer.publicKey;
        return { transaction, newAuthority };
    }

    async createSessionTxn(params: Parameters<typeof this.createSession>[0]): Promise<{
        transaction: Transaction;
        sessionPda: PublicKey;
        sessionKeypair: Keypair | null;
    }> {
        const { ix, sessionPda, sessionKeypair } = await this.createSession(params);
        const transaction = new Transaction().add(ix);
        transaction.feePayer = params.payer.publicKey;
        return { transaction, sessionPda, sessionKeypair };
    }

    async executeTxn(params: Parameters<typeof this.execute>[0]): Promise<Transaction> {
        const ix = await this.execute(params);
        const transaction = new Transaction().add(ix);
        transaction.feePayer = params.payer.publicKey;
        return transaction;
    }

    // ─── Discovery helpers ────────────────────────────────────────────────────

    /**
     * Finds all Wallet PDAs associated with a given Ed25519 public key.
     */
    static async findWalletByOwner(
        connection: Connection,
        owner: PublicKey,
        programId: PublicKey = PROGRAM_ID
    ): Promise<PublicKey[]> {
        const accounts = await connection.getProgramAccounts(programId, {
            filters: [{ dataSize: AUTHORITY_ACCOUNT_ED25519_SIZE }], // Ed25519 authority size
        });


        const results: PublicKey[] = [];
        for (const a of accounts) {
            const data = a.account.data;
            if (data[0] === 2 && data[1] === 0) { // disc=2 (Authority), type=0 (Ed25519)
                const storedPubkey = data.subarray(48, 80);
                if (Buffer.compare(storedPubkey, owner.toBuffer()) === 0) {
                    results.push(new PublicKey(data.subarray(16, 48)));
                }
            }
        }
        return results;
    }

    /**
     * Finds all Wallet PDAs associated with a Secp256r1 credential hash.
     */
    static async findWalletByCredentialHash(
        connection: Connection,
        credentialHash: Uint8Array,
        programId: PublicKey = PROGRAM_ID
    ): Promise<PublicKey[]> {
        const accounts = await connection.getProgramAccounts(programId, {
            filters: [{ dataSize: AUTHORITY_ACCOUNT_SECP256R1_SIZE }], // Secp256r1 authority size
        });


        const results: PublicKey[] = [];
        for (const a of accounts) {
            const data = a.account.data;
            if (data[0] === 2 && data[1] === 1) { // disc=2 (Authority), type=1 (Secp256r1)
                const storedHash = data.subarray(48, 80);
                if (Buffer.compare(storedHash, Buffer.from(credentialHash)) === 0) {
                    results.push(new PublicKey(data.subarray(16, 48)));
                }
            }
        }
        return results;
    }

    /**
     * Finds all Authority PDA records associated with a given Secp256r1 credential hash.
     *
     * Unlike `findWalletByCredentialHash` which only returns Wallet PDAs, this method
     * returns rich authority metadata — useful for credential-based wallet recovery flows.
     *
     * Account memory layout (Authority PDA):
     * - byte 0: discriminator (2 = Authority)
     * - byte 1: authority_type (0 = Ed25519, 1 = Secp256r1)
     * - byte 2: role (0 = Owner, 1 = Admin, 2 = Spender)
     * - bytes 16–48: wallet PDA
     * - bytes 48–80: credential_id_hash (Secp256r1) or pubkey (Ed25519)
     */
    static async findAllAuthoritiesByCredentialHash(
        connection: Connection,
        credentialHash: Uint8Array,
        programId: PublicKey = PROGRAM_ID
    ): Promise<Array<{
        authority: PublicKey;
        wallet: PublicKey;
        role: number;
        authorityType: number;
    }>> {
        const accounts = await connection.getProgramAccounts(programId, {
            filters: [
                { dataSize: AUTHORITY_ACCOUNT_SECP256R1_SIZE },          // Secp256r1 authority size
                { memcmp: { offset: 0, bytes: bs58.encode(Buffer.from([DISCRIMINATOR_AUTHORITY])) } },  // disc = Authority
                { memcmp: { offset: 48, bytes: bs58.encode(Buffer.from(credentialHash)) } }, // credentialHash
            ],
        });


        return accounts.map(a => {
            const data = a.account.data;
            return {
                authority: a.pubkey,
                wallet: new PublicKey(data.subarray(16, 48)),
                role: data[2],
                authorityType: data[1],
            };
        });
    }
}
