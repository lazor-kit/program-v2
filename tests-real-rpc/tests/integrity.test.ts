
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
} from "@solana/kit";
import { setupTest, processInstruction, type TestContext } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../sdk/lazorkit-ts/src";
import * as crypto from "crypto";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

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
    let context: TestContext;
    let client: any;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    async function getRawAccountData(address: Address): Promise<Buffer> {
        const { value: acc } = await context.rpc.getAccountInfo(address, { encoding: 'base64' }).send();
        return Buffer.from(acc!.data[0], 'base64');
    }

    it("Ed25519: pubkey stored at correct offset", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = await generateKeyPairSigner();
        const ownerPubkeyBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerPubkeyBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
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
        expect(Uint8Array.from(storedWallet)).toEqual(Uint8Array.from(getAddressEncoder().encode(walletPda)));

        // Ed25519 pubkey at DATA_OFFSET
        const storedPubkey = data.subarray(DATA_OFFSET, DATA_OFFSET + 32);
        expect(Uint8Array.from(storedPubkey)).toEqual(ownerPubkeyBytes);
    });

    it("Secp256r1: credential_id_hash + pubkey stored at correct offsets", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const credentialIdHash = new Uint8Array(crypto.randomBytes(32));
        const p256Pubkey = new Uint8Array(33); // compressed P-256 key
        p256Pubkey[0] = 0x02; // valid prefix
        crypto.randomBytes(32).copy(p256Pubkey, 1);

        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
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
        expect(Uint8Array.from(storedCredHash)).toEqual(Uint8Array.from(credentialIdHash));

        // pubkey at SECP256R1_PUBKEY_OFFSET (33 bytes compressed)
        const storedPubkey = data.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33);
        expect(Uint8Array.from(storedPubkey)).toEqual(Uint8Array.from(p256Pubkey));
    });

    it("Multiple Secp256r1 authorities with different credential_id_hash", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        // Create wallet with Ed25519 owner first
        const owner = await generateKeyPairSigner();
        const ownerPubkeyBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerPda, ownerBump] = await findAuthorityPda(walletPda, ownerPubkeyBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerPda,
            userSeed,
            authType: 0,
            authBump: ownerBump,
            authPubkey: ownerPubkeyBytes,
            credentialHash: new Uint8Array(32),
        }));

        // Add Passkey 1
        const credHash1 = new Uint8Array(crypto.randomBytes(32));
        const pubkey1 = new Uint8Array(33); pubkey1[0] = 0x02; crypto.randomBytes(32).copy(pubkey1, 1);
        const [authPda1] = await findAuthorityPda(walletPda, credHash1);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerPda,
            newAuthority: authPda1,
            authType: 1,
            newRole: 1, // Admin
            authPubkey: pubkey1,
            credentialHash: credHash1,
            authorizerSigner: owner,
        }), [owner]);

        // Add Passkey 2 (same domain, different credential)
        const credHash2 = new Uint8Array(crypto.randomBytes(32));
        const pubkey2 = new Uint8Array(33); pubkey2[0] = 0x03; crypto.randomBytes(32).copy(pubkey2, 1);
        const [authPda2] = await findAuthorityPda(walletPda, credHash2);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerPda,
            newAuthority: authPda2,
            authType: 1,
            newRole: 2, // Spender
            authPubkey: pubkey2,
            credentialHash: credHash2,
            authorizerSigner: owner,
        }), [owner]);

        // PDAs must be unique
        expect(authPda1).not.toEqual(authPda2);

        // Verify Passkey 1 data
        const data1 = await getRawAccountData(authPda1);
        expect(data1[1]).toBe(1); // Secp256r1
        expect(data1[2]).toBe(1); // Admin
        expect(Uint8Array.from(data1.subarray(DATA_OFFSET, DATA_OFFSET + 32))).toEqual(Uint8Array.from(credHash1));
        expect(Uint8Array.from(data1.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33))).toEqual(Uint8Array.from(pubkey1));

        // Verify Passkey 2 data
        const data2 = await getRawAccountData(authPda2);
        expect(data2[1]).toBe(1); // Secp256r1
        expect(data2[2]).toBe(2); // Spender
        expect(Uint8Array.from(data2.subarray(DATA_OFFSET, DATA_OFFSET + 32))).toEqual(Uint8Array.from(credHash2));
        expect(Uint8Array.from(data2.subarray(SECP256R1_PUBKEY_OFFSET, SECP256R1_PUBKEY_OFFSET + 33))).toEqual(Uint8Array.from(pubkey2));
    });
});
