import {
    Connection,
    Keypair,
    PublicKey,
    Transaction,
    TransactionInstruction,
    SystemProgram,
    SYSVAR_RENT_PUBKEY,
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

// --- Enums ---
export enum AuthType {
    Ed25519 = 0,
    Secp256r1 = 1
}

export { Role };

// 1. Create Wallet: Distinguish Ed25519 and Secp256r1
export type CreateWalletParams =
    | {
        payer: Keypair;
        authType: AuthType.Ed25519;
        owner: PublicKey;
        userSeed?: Uint8Array;
    }
    | {
        payer: Keypair;
        authType?: AuthType.Secp256r1; // <--- Optional for default
        pubkey: Uint8Array; // 33 bytes compressed
        credentialHash: Uint8Array; // 32 bytes
        userSeed?: Uint8Array;
    };

// 2. Authorizer Signer (Admin/Owner)
export type AdminSignerOptions =
    | {
        adminType: AuthType.Ed25519;
        adminSigner: Keypair;
    }
    | {
        adminType?: AuthType.Secp256r1; // <--- Optional for default
        adminCredentialHash: Uint8Array; // 32 bytes to derive PDA
        adminSignature: Uint8Array; // 64/65 bytes signature 
    };

export class LazorClient {
    public client: LazorWeb3Client;

    constructor(
        public connection: Connection,
        public programId: PublicKey = PROGRAM_ID
    ) {
        this.client = new LazorWeb3Client(programId);
    }

    /**
     * Send Transaction supporting multiple Instructions and Signers array
     */
    public async sendTx(
        instructions: TransactionInstruction[],
        signers: Keypair[]
    ): Promise<string> {
        const tx = new Transaction();
        instructions.forEach(ix => tx.add(ix));
        const { blockhash } = await this.connection.getLatestBlockhash();
        tx.recentBlockhash = blockhash;
        tx.feePayer = signers[0].publicKey;
        signers.forEach(s => tx.partialSign(s));
        const signature = await this.connection.sendRawTransaction(tx.serialize());
        await this.connection.confirmTransaction(signature, "confirmed");
        return signature;
    }

    private getShardId(pubkey: PublicKey): number {
        return pubkey.toBytes().reduce((a, b) => a + b, 0) % 16;
    }

    // 1. Create Wallet (Simplified parameters)
    async createWallet(params: CreateWalletParams): Promise<{ ix: TransactionInstruction, walletPda: PublicKey, authorityPda: PublicKey, userSeed: Uint8Array }> {
        const userSeed = params.userSeed || crypto.getRandomValues(new Uint8Array(32));
        const [walletPda] = findWalletPda(userSeed, this.programId);
        const [vaultPda] = findVaultPda(walletPda, this.programId);

        let authorityPda: PublicKey;
        let authBump: number;
        let authPubkey: Uint8Array;
        let credentialHash: Uint8Array = new Uint8Array(32);

        const authType = params.authType ?? AuthType.Secp256r1;

        if (params.authType === AuthType.Ed25519) { 
            authPubkey = params.owner.toBytes();
            [authorityPda, authBump] = findAuthorityPda(walletPda, authPubkey, this.programId);
        } else { 
            const p = params as { pubkey: Uint8Array, credentialHash: Uint8Array };
            authPubkey = p.pubkey;
            credentialHash = p.credentialHash;
            [authorityPda, authBump] = findAuthorityPda(walletPda, credentialHash, this.programId);
        }

        const [configPda] = findConfigPda(this.programId);
        const shardId = this.getShardId(params.payer.publicKey);
        const [treasuryShard] = findTreasuryShardPda(shardId, this.programId);

        const ix = this.client.createWallet({
            config: configPda,
            treasuryShard: treasuryShard,
            payer: params.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: authorityPda,
            userSeed,
            authType: authType,
            authBump: authBump,
            authPubkey: authPubkey,
            credentialHash: credentialHash
        });

        return { ix, walletPda, authorityPda, userSeed };
    }

    // 2. Manage Authority
    async addAuthority(params: {
        payer: Keypair;
        walletPda: PublicKey;
        newAuthorityPubkey: Uint8Array;
        authType?: AuthType;
        role?: Role;
        credentialHash?: Uint8Array;
        adminAuthorityPda?: PublicKey; // <--- Add optional override
    } & AdminSignerOptions): Promise<{ ix: TransactionInstruction, newAuthority: PublicKey }> {
        const [configPda] = findConfigPda(this.programId);
        const shardId = this.getShardId(params.payer.publicKey);
        const [treasuryShard] = findTreasuryShardPda(shardId, this.programId);

        const authType = params.authType ?? AuthType.Secp256r1;
        const role = params.role ?? Role.Spender;
        const adminType = params.adminType ?? AuthType.Secp256r1;

        const idSeed = authType === AuthType.Secp256r1 
            ? params.credentialHash || new Uint8Array(32) 
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
            authType: authType,
            newRole: role,
            authPubkey: params.newAuthorityPubkey,
            credentialHash: params.credentialHash,
            authorizerSigner: adminType === AuthType.Ed25519 ? (params as any).adminSigner.publicKey : undefined
        });

        return { ix, newAuthority };
    }

    // 3. Create Session
    async createSession(params: {
        payer: Keypair;
        walletPda: PublicKey;
        sessionKey: PublicKey;
        expiresAt?: bigint;
    } & AdminSignerOptions): Promise<{ ix: TransactionInstruction, sessionPda: PublicKey }> {
        const [configPda] = findConfigPda(this.programId);
        const shardId = this.getShardId(params.payer.publicKey);
        const [treasuryShard] = findTreasuryShardPda(shardId, this.programId);

        const expiresAt = params.expiresAt ?? BigInt(Math.floor(Date.now() / 1000) + 3600);
        const adminType = params.adminType ?? AuthType.Secp256r1;

        const [sessionPda] = findSessionPda(params.walletPda, params.sessionKey, this.programId);

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
            sessionKey: Array.from(params.sessionKey.toBytes()),
            expiresAt: expiresAt,
            authorizerSigner: adminType === AuthType.Ed25519 ? (params as any).adminSigner.publicKey : undefined
        });

        return { ix, sessionPda };
    }

    // 4. Execute Instructions
    async execute(params: {
        payer: Keypair;
        walletPda: PublicKey;
        authorityPda: PublicKey; 
        innerInstructions: TransactionInstruction[];
        signer?: Keypair; 
        signature?: Uint8Array; 
        vaultPda?: PublicKey; // <--- Add optional override
    }): Promise<TransactionInstruction> {
        const [configPda] = findConfigPda(this.programId);
        const shardId = this.getShardId(params.payer.publicKey);
        const [treasuryShard] = findTreasuryShardPda(shardId, this.programId);
        const [vaultPda] = params.vaultPda ? [params.vaultPda] : findVaultPda(params.walletPda, this.programId);

        const ix = this.client.buildExecute({
            config: configPda,
            treasuryShard,
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            authority: params.authorityPda,
            vault: vaultPda,
            innerInstructions: params.innerInstructions,
            authorizerSigner: params.signer ? params.signer.publicKey : undefined
        });

        if (params.signature) {
             const newData = Buffer.alloc(ix.data.length + params.signature.length);
             ix.data.copy(newData);
             newData.set(params.signature, ix.data.length);
             ix.data = newData;
        }

        return ix;
    }

    // 5. Discover Wallets
    static async findWalletByOwner(
        connection: Connection,
        owner: PublicKey,
        programId: PublicKey = PROGRAM_ID
    ): Promise<PublicKey[]> {
         const accounts = await connection.getProgramAccounts(programId, {
             filters: [{ dataSize: 48 + 32 }] // Type: Ed25519 Size
         });
         
         const results: PublicKey[] = [];
         for (const a of accounts) {
             const data = a.account.data;
             if (data[0] === 2 && data[1] === 0) { // Disc=2, Type=0 (Ed25519)
                 const storedPubkey = data.subarray(48, 80);
                 if (Buffer.compare(storedPubkey, owner.toBuffer()) === 0) {
                     results.push(new PublicKey(data.subarray(16, 48)));
                 }
             }
         }
         return results;
    }

    static async findWalletByCredentialHash(
        connection: Connection,
        credentialHash: Uint8Array,
        programId: PublicKey = PROGRAM_ID
    ): Promise<PublicKey[]> {
         const accounts = await connection.getProgramAccounts(programId, {
             filters: [{ dataSize: 48 + 65 }] // Secp size (48 + 32 + 33)
         });
         
         const results: PublicKey[] = [];
         for (const a of accounts) {
             const data = a.account.data;
             if (data[0] === 2 && data[1] === 1) { // Disc=2, Type=1 (Secp256r1)
                 const storedHash = data.subarray(48, 80);
                 if (Buffer.compare(storedHash, Buffer.from(credentialHash)) === 0) {
                     results.push(new PublicKey(data.subarray(16, 48)));
                 }
             }
         }
         return results;
    }
    // 6. Remove Authority
    async removeAuthority(params: {
        payer: Keypair;
        walletPda: PublicKey;
        authorityToRemovePda: PublicKey;
        refundDestination: PublicKey; // Refund destination on close account
    } & AdminSignerOptions): Promise<TransactionInstruction> {
        const [configPda] = findConfigPda(this.programId);
        const shardId = this.getShardId(params.payer.publicKey);
        const [treasuryShard] = findTreasuryShardPda(shardId, this.programId);

        let adminAuthority: PublicKey;
        if (params.adminType === AuthType.Ed25519) {
            [adminAuthority] = findAuthorityPda(params.walletPda, params.adminSigner.publicKey.toBytes(), this.programId);
        } else {
             [adminAuthority] = findAuthorityPda(params.walletPda, params.adminCredentialHash, this.programId);
        }

        const ix = this.client.removeAuthority({
            config: configPda,
            treasuryShard,
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            adminAuthority,
            targetAuthority: params.authorityToRemovePda,
            refundDestination: params.refundDestination,
            authorizerSigner: params.adminType === AuthType.Ed25519 ? params.adminSigner.publicKey : undefined
        });

        return ix;
    }

    // 7. Cleanup
    async closeWallet(params: {
        payer: Keypair;
        walletPda: PublicKey;
        destination: PublicKey;
        vaultPda?: PublicKey;           // <--- Add optional override
        adminAuthorityPda?: PublicKey;   // <--- Add optional override
    } & AdminSignerOptions): Promise<TransactionInstruction> {
        const [vaultPda] = params.vaultPda ? [params.vaultPda] : findVaultPda(params.walletPda, this.programId);
        
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
            ownerAuthority: ownerAuthority,
            destination: params.destination,
            ownerSigner: params.adminType === AuthType.Ed25519 ? params.adminSigner.publicKey : undefined
        });

        // Add SystemProgram for closing support
        ix.keys.push({
            pubkey: SystemProgram.programId,
            isWritable: false,
            isSigner: false,
        });

        return ix;
    }

    // 8. Admin Methods
    async initializeConfig(params: {
        admin: Keypair;
        walletFee: bigint;
        actionFee: bigint;
        numShards: number;
    }): Promise<TransactionInstruction> {
        const [configPda] = findConfigPda(this.programId);
        const ix = this.client.initializeConfig({
            admin: params.admin.publicKey,
            config: configPda,
            walletFee: params.walletFee,
            actionFee: params.actionFee,
            numShards: params.numShards
        });

        return ix;
    }

    async initTreasuryShard(params: {
        payer: Keypair;
        shardId: number;
    }): Promise<TransactionInstruction> {
        const [configPda] = findConfigPda(this.programId);
        const [treasuryShard] = findTreasuryShardPda(params.shardId, this.programId);

        const ix = this.client.initTreasuryShard({
            payer: params.payer.publicKey,
            config: configPda,
            treasuryShard,
            shardId: params.shardId
        });

        return ix;
    }

    async sweepTreasury(params: {
        admin: Keypair;
        shardId: number;
        destination: PublicKey;
    }): Promise<TransactionInstruction> {
        const [configPda] = findConfigPda(this.programId);
        const [treasuryShard] = findTreasuryShardPda(params.shardId, this.programId);

        const ix = this.client.sweepTreasury({
            admin: params.admin.publicKey,
            config: configPda,
            treasuryShard,
            destination: params.destination,
            shardId: params.shardId
        });

        return ix;
    }

    // 9. Transfer Ownership
    async transferOwnership(params: {
        payer: Keypair;
        walletPda: PublicKey;
        currentOwnerAuthority: PublicKey;
        newOwnerAuthority: PublicKey;
        authType: AuthType;
        authPubkey: Uint8Array;
        credentialHash?: Uint8Array;
        signer?: Keypair; // optional authorizer signer
    }): Promise<TransactionInstruction> {
        const [configPda] = findConfigPda(this.programId);
        const shardId = this.getShardId(params.payer.publicKey);
        const [treasuryShard] = findTreasuryShardPda(shardId, this.programId);

        const ix = this.client.transferOwnership({
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            currentOwnerAuthority: params.currentOwnerAuthority,
            newOwnerAuthority: params.newOwnerAuthority,
            config: configPda,
            treasuryShard,
            authType: params.authType,
            authPubkey: params.authPubkey,
            credentialHash: params.credentialHash,
            authorizerSigner: params.signer ? params.signer.publicKey : undefined
        });

        return ix;
    }

    // 10. Close Session
    async closeSession(params: {
        payer: Keypair;
        walletPda: PublicKey;
        sessionPda: PublicKey;
        configPda?: PublicKey; // <--- Add optional override
        authorizer?: {
            authorizerPda: PublicKey;
            signer: Keypair;
        };
    }): Promise<TransactionInstruction> {
        const [configPda] = params.configPda ? [params.configPda] : findConfigPda(this.programId);
        
        const ix = this.client.closeSession({
            config: configPda,
            payer: params.payer.publicKey,
            wallet: params.walletPda,
            session: params.sessionPda,
            authorizer: params.authorizer ? params.authorizer.authorizerPda : undefined,
            authorizerSigner: params.authorizer ? params.authorizer.signer.publicKey : undefined
        });
        
        return ix;
    }

    // === Layer 2: Transaction Builders ===

    async createWalletTxn(params: CreateWalletParams): Promise<{ transaction: Transaction, walletPda: PublicKey, authorityPda: PublicKey, userSeed: Uint8Array }> {
        const { ix, walletPda, authorityPda, userSeed } = await this.createWallet(params);
        const transaction = new Transaction().add(ix);
        transaction.feePayer = params.payer.publicKey;
        return { transaction, walletPda, authorityPda, userSeed };
    }

    async addAuthorityTxn(params: Parameters<typeof this.addAuthority>[0]): Promise<{ transaction: Transaction, newAuthority: PublicKey }> {
        const { ix, newAuthority } = await this.addAuthority(params);
        const transaction = new Transaction().add(ix);
        transaction.feePayer = params.payer.publicKey;
        return { transaction, newAuthority };
    }

    async createSessionTxn(params: Parameters<typeof this.createSession>[0]): Promise<{ transaction: Transaction, sessionPda: PublicKey }> {
        const { ix, sessionPda } = await this.createSession(params);
        const transaction = new Transaction().add(ix);
        transaction.feePayer = params.payer.publicKey;
        return { transaction, sessionPda };
    }

    async executeTxn(params: Parameters<typeof this.execute>[0]): Promise<Transaction> {
        const ix = await this.execute(params);
        const transaction = new Transaction().add(ix);
        transaction.feePayer = params.payer.publicKey;
        return transaction;
    }
}
