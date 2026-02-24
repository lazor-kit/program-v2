
import { describe, it, expect, beforeAll } from "vitest";
import { PublicKey, Keypair } from "@solana/web3.js";
import { Address } from "@solana/kit";
import { setupTest, processInstruction } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../src";
import * as crypto from "crypto";

/**
 * AuthorityAccountHeader layout (48 bytes):
 *   discriminator(1) + authority_type(1) + role(1) + bump(1) +
 *   version(1) + _padding(3) + counter(8) + wallet(32) = 48
 *
 * After header:
 *   Ed25519:    [pubkey(32)]
 *   Secp256r1:  [credential_id_hash(32)] [pubkey(33)]
 *
 * Both authority types start variable data at offset 48.
 */
const HEADER_SIZE = 48;
const DATA_OFFSET = HEADER_SIZE;                   // offset 48 for both types
const SECP256R1_PUBKEY_OFFSET = DATA_OFFSET + 32;  // offset 80

describe("Contract Data Integrity", () => {
    let context: any;
    let client: any;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    async function getRawAccountData(address: Address): Promise<Buffer> {
        const acc = await context.banksClient.getAccount(new PublicKey(address));
        return Buffer.from(acc!.data);
    }

    it("Ed25519: pubkey stored at correct offset", async () => {
        const userSeed = new Uint8Array(32).fill(100);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = Keypair.generate();
        const ownerPubkeyBytes = owner.publicKey.toBytes();
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerPubkeyBytes);

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerPubkeyBytes,
            credentialHash: new Uint8Array(32),
        }));

        const data = await getRawAccountData(authPda);

        // Header checks
        expect(data[0]).toBe(2);  // discriminator = Authority
        expect(data[1]).toBe(0);  // authority_type = Ed25519
        expect(data[2]).toBe(0);  // role = Owner

        // Wallet pubkey in header (at offset 16 = 1+1+1+1+1+3+8)
        const storedWallet = data.subarray(16, 48);
        expect(Buffer.from(storedWallet)).toEqual(Buffer.from(new PublicKey(walletPda).toBytes()));

        // Ed25519 pubkey at DATA_OFFSET
        const storedPubkey = data.subarray(DATA_OFFSET, DATA_OFFSET + 32);
        expect(Buffer.from(storedPubkey)).toEqual(Buffer.from(ownerPubkeyBytes));
    });

    it("Secp256r1: credential_id_hash + pubkey stored at correct offsets", async () => {
        const userSeed = new Uint8Array(32).fill(101);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const credentialIdHash = Buffer.from(crypto.randomBytes(32));
        const p256Pubkey = Buffer.alloc(33); // compressed P-256 key
        p256Pubkey[0] = 0x02; // valid prefix
        crypto.randomBytes(32).copy(p256Pubkey, 1);

        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1, // Secp256r1
            authBump,
            authPubkey: p256Pubkey,
            credentialHash: credentialIdHash,
        }));

        const data = await getRawAccountData(authPda);

        // Header checks
        expect(data[0]).toBe(2);  // discriminator = Authority
        expect(data[1]).toBe(1);  // authority_type = Secp256r1
        expect(data[2]).toBe(0);  // role = Owner

        // credential_id_hash at DATA_OFFSET
        const storedCredHash = data.subarray(DATA_OFFSET, DATA_OFFSET + 32);
        expect(Buffer.from(storedCredHash)).toEqual(credentialIdHash);

        // pubkey at SECP256R1_PUBKEY_OFFSET (33 bytes compressed)
        const storedPubkey = data.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33);
        expect(Buffer.from(storedPubkey)).toEqual(p256Pubkey);
    });

    it("Multiple Secp256r1 authorities with different credential_id_hash", async () => {
        const userSeed = new Uint8Array(32).fill(102);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        // Create wallet with Ed25519 owner first
        const owner = Keypair.generate();
        const [ownerPda, ownerBump] = await findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerPda,
            userSeed,
            authType: 0,
            authBump: ownerBump,
            authPubkey: owner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        }));

        // Add Passkey 1
        const credHash1 = Buffer.from(crypto.randomBytes(32));
        const pubkey1 = Buffer.alloc(33); pubkey1[0] = 0x02; crypto.randomBytes(32).copy(pubkey1, 1);
        const [authPda1] = await findAuthorityPda(walletPda, credHash1);

        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerPda,
            newAuthority: authPda1,
            authType: 1,
            newRole: 1, // Admin
            authPubkey: pubkey1,
            credentialHash: credHash1,
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        // Add Passkey 2 (same domain, different credential)
        const credHash2 = Buffer.from(crypto.randomBytes(32));
        const pubkey2 = Buffer.alloc(33); pubkey2[0] = 0x03; crypto.randomBytes(32).copy(pubkey2, 1);
        const [authPda2] = await findAuthorityPda(walletPda, credHash2);

        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerPda,
            newAuthority: authPda2,
            authType: 1,
            newRole: 2, // Spender
            authPubkey: pubkey2,
            credentialHash: credHash2,
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        // PDAs must be unique
        expect(authPda1).not.toEqual(authPda2);

        // Verify Passkey 1 data
        const data1 = await getRawAccountData(authPda1);
        expect(data1[1]).toBe(1); // Secp256r1
        expect(data1[2]).toBe(1); // Admin
        expect(Buffer.from(data1.subarray(DATA_OFFSET, DATA_OFFSET + 32))).toEqual(credHash1);
        expect(Buffer.from(data1.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33))).toEqual(pubkey1);

        // Verify Passkey 2 data
        const data2 = await getRawAccountData(authPda2);
        expect(data2[1]).toBe(1); // Secp256r1
        expect(data2[2]).toBe(2); // Spender
        expect(Buffer.from(data2.subarray(DATA_OFFSET, DATA_OFFSET + 32))).toEqual(credHash2);
        expect(Buffer.from(data2.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33))).toEqual(pubkey2);
    });
});
