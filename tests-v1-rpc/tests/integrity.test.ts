import { expect, describe, it, beforeAll } from "vitest";
import { Keypair, PublicKey } from "@solana/web3.js";
import { setupTest, sendTx, getRandomSeed, type TestContext } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda, LazorClient, AuthType, Role } from "@lazorkit/solita-client";



const HEADER_SIZE = 48;
const DATA_OFFSET = HEADER_SIZE;                   
const SECP256R1_PUBKEY_OFFSET = DATA_OFFSET + 32;  

describe("Contract Data Integrity", () => {
    let ctx: TestContext;
    beforeAll(async () => {
        ctx = await setupTest();
    });

    async function getRawAccountData(address: PublicKey): Promise<Buffer> {
        const acc = await ctx.connection.getAccountInfo(address);
        if (!acc) throw new Error(`Account ${address.toBase58()} not found`);
        return acc.data; 
    }

    it("Ed25519: pubkey stored at correct offset", async () => {
        const userSeed = getRandomSeed();
        const owner = Keypair.generate();
        const ownerPubkeyBytes = owner.publicKey.toBytes();

        const { ix, walletPda, authorityPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey,
            userSeed
        });
        await sendTx(ctx, [ix]);

        const data = await getRawAccountData(authorityPda);

        // Header checks
        expect(data[0]).toBe(2);  // discriminator = Authority
        expect(data[1]).toBe(0);  // authority_type = Ed25519
        expect(data[2]).toBe(0);  // role = Owner

        // Wallet pubkey in header (at offset 16)
        const storedWallet = data.subarray(16, 48);
        expect(Uint8Array.from(storedWallet)).toEqual(walletPda.toBytes());

        // Ed25519 pubkey at DATA_OFFSET
        const storedPubkey = data.subarray(DATA_OFFSET, DATA_OFFSET + 32);
        expect(Uint8Array.from(storedPubkey)).toEqual(ownerPubkeyBytes);
    });

    it("Secp256r1: credential_id_hash + pubkey stored at correct offsets", async () => {
        const userSeed = getRandomSeed();

        const credentialIdHash = getRandomSeed();
        const p256Pubkey = new Uint8Array(33); 
        p256Pubkey[0] = 0x02; 
        crypto.getRandomValues(p256Pubkey.subarray(1));

        const { ix, walletPda, authorityPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Secp256r1,
            pubkey: p256Pubkey,
            credentialHash: credentialIdHash,
            userSeed
        });
        await sendTx(ctx, [ix]);

        const data = await getRawAccountData(authorityPda);

        // Header checks
        expect(data[0]).toBe(2);  // discriminator = Authority
        expect(data[1]).toBe(1);  // authority_type = Secp256r1
        expect(data[2]).toBe(0);  // role = Owner

        // credential_id_hash at DATA_OFFSET
        const storedCredHash = data.subarray(DATA_OFFSET, DATA_OFFSET + 32);
        expect(Uint8Array.from(storedCredHash)).toEqual(credentialIdHash);

        // pubkey at SECP256R1_PUBKEY_OFFSET (33 bytes compressed)
        const storedPubkey = data.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33);
        expect(Uint8Array.from(storedPubkey)).toEqual(p256Pubkey);
    });

    it("Multiple Secp256r1 authorities with different credential_id_hash", async () => {
        const userSeed = getRandomSeed();

        // Create wallet with Ed25519 owner first
        const owner = Keypair.generate();
        const ownerPubkeyBytes = owner.publicKey.toBytes();

        const { ix, walletPda, authorityPda: ownerPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey,
            userSeed
        });
        await sendTx(ctx, [ix]);

        // Add Passkey 1
        const credHash1 = getRandomSeed();
        const pubkey1 = new Uint8Array(33); pubkey1[0] = 0x02; crypto.getRandomValues(pubkey1.subarray(1));

        const { ix: ixAdd1, newAuthority: authPda1 } = await ctx.highClient.addAuthority({
            payer: ctx.payer,
            walletPda,
            adminType: AuthType.Ed25519,
            adminSigner: owner,
            newAuthorityPubkey: pubkey1,
            authType: AuthType.Secp256r1,
            role: Role.Admin,
            credentialHash: credHash1
        });
        await sendTx(ctx, [ixAdd1], [owner]);

        // Add Passkey 2
        const credHash2 = getRandomSeed();
        const pubkey2 = new Uint8Array(33); pubkey2[0] = 0x03; crypto.getRandomValues(pubkey2.subarray(1));

        const { ix: ixAdd2, newAuthority: authPda2 } = await ctx.highClient.addAuthority({
            payer: ctx.payer,
            walletPda,
            adminType: AuthType.Ed25519,
            adminSigner: owner,
            newAuthorityPubkey: pubkey2,
            authType: AuthType.Secp256r1,
            role: Role.Spender,
            credentialHash: credHash2
        });
        await sendTx(ctx, [ixAdd2], [owner]);

        // PDAs must be unique
        expect(authPda1.toBase58()).not.toEqual(authPda2.toBase58());

        // Verify Passkey 1 data
        const data1 = await getRawAccountData(authPda1);
        expect(data1[1]).toBe(1); // Secp256r1
        expect(data1[2]).toBe(1); // Admin
        expect(Uint8Array.from(data1.subarray(DATA_OFFSET, DATA_OFFSET + 32))).toEqual(credHash1);
        expect(Uint8Array.from(data1.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33))).toEqual(pubkey1);

        // Verify Passkey 2 data
        const data2 = await getRawAccountData(authPda2);
        expect(data2[1]).toBe(1); // Secp256r1
        expect(data2[2]).toBe(2); // Spender
        expect(Uint8Array.from(data2.subarray(DATA_OFFSET, DATA_OFFSET + 32))).toEqual(credHash2);
        expect(Uint8Array.from(data2.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33))).toEqual(pubkey2);
    });
});
