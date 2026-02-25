
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    type TransactionSigner
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../../sdk/lazorkit-ts/src";
import { LazorClient } from "../../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("Instruction: CreateWallet", () => {
    let context: TestContext;
    let client: LazorClient;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    it("Success: Create wallet with Ed25519 owner", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        }));

        // Verify state
        const walletAcc = await client.getWallet(walletPda);
        expect(walletAcc.discriminator).toBe(1); // Wallet
        expect(walletAcc.version).toBe(1);

        const authAcc = await client.getAuthority(authPda);
        expect(authAcc.discriminator).toBe(2); // Authority
        expect(authAcc.role).toBe(0); // Owner
        expect(authAcc.authorityType).toBe(0); // Ed25519
    });

    it("Failure: Account already initialized", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        const ix = client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        });

        await processInstruction(context, ix);

        // Try again
        const result = await tryProcessInstruction(context, ix);
        // Standard Solana error for already in use:
        expect(result.result).toMatch(/already|in use|simulation failed/i);
    });

    it("Failure: Invalid PDA seeds (wrong authority PDA)", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));

        // Use a different seed for the PDA than what's in instruction data
        const [wrongAuthPda] = await findAuthorityPda(walletPda, new Uint8Array(32).fill(99));
        const [, actualBump] = await findAuthorityPda(walletPda, ownerBytes);

        const result = await tryProcessInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: wrongAuthPda,
            userSeed,
            authType: 0,
            authBump: actualBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        }));

        expect(result.result).toMatch(/seeds|valid address|simulation failed/i);
    });

    // --- Category 2: SDK Encoding Correctness ---

    it("Encoding: Ed25519 CreateWallet data matches expected binary layout", async () => {
        const userSeed = getRandomSeed();
        const owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        const ix = client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        });

        const data = Buffer.from(ix.data);
        // Layout: [disc(1)][userSeed(32)][authType(1)][authBump(1)][padding(6)][pubkey(32)]
        // Total: 1 + 32 + 1 + 1 + 6 + 32 = 73
        expect(data.length).toBe(73);
        expect(data[0]).toBe(0);                                              // discriminator
        expect(Uint8Array.from(data.subarray(1, 33))).toEqual(userSeed); // userSeed
        expect(data[33]).toBe(0);                                             // authType = Ed25519
        expect(data[34]).toBe(authBump);                                      // bump
        expect(Uint8Array.from(data.subarray(35, 41))).toEqual(new Uint8Array(6).fill(0)); // padding
        expect(Uint8Array.from(data.subarray(41, 73))).toEqual(ownerBytes);
    });

    it("Encoding: Secp256r1 CreateWallet data matches expected binary layout", async () => {
        const userSeed = getRandomSeed();
        const credentialIdHash = new Uint8Array(32).fill(0xAA);
        const p256Pubkey = new Uint8Array(33).fill(0xBB);
        p256Pubkey[0] = 0x02;

        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        const ix = client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1,
            authBump,
            authPubkey: p256Pubkey,
            credentialHash: credentialIdHash,
        });

        const data = Buffer.from(ix.data);
        // Layout: [disc(1)][userSeed(32)][authType(1)][authBump(1)][padding(6)][credIdHash(32)][pubkey(33)]
        // Total: 1 + 32 + 1 + 1 + 6 + 32 + 33 = 106
        expect(data.length).toBe(106);
        expect(data[0]).toBe(0);                                                 // discriminator
        expect(data[33]).toBe(1);                                                // authType = Secp256r1
        expect(Uint8Array.from(data.subarray(41, 73))).toEqual(credentialIdHash);    // credential_id_hash
        expect(Uint8Array.from(data.subarray(73, 106))).toEqual(p256Pubkey);         // pubkey
    });
});
