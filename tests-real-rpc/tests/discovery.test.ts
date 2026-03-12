import { describe, it, expect, beforeAll } from "vitest";
import { setupTest, processInstruction, type TestContext } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../sdk/lazorkit-ts/src";
import crypto from "crypto";

describe("Recovery by Credential Hash", () => {
    let context: TestContext;
    let client: any;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    }, 30000);

    it("Should discover a wallet by its credential hash", async () => {
        // 1. Setup random data
        const userSeed = new Uint8Array(32);
        crypto.getRandomValues(userSeed);

        const credentialIdHash = new Uint8Array(32);
        crypto.getRandomValues(credentialIdHash);

        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        // Dummy Secp256r1 pubkey (33 bytes)
        const authPubkey = new Uint8Array(33).fill(7);

        // 2. Create the wallet
        console.log("Creating wallet for discovery test...");
        const createIx = client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            userSeed,
            authType: 1, // Secp256r1
            authBump,
            authPubkey,
            credentialHash: credentialIdHash,
        });

        await processInstruction(context, createIx, [context.payer]);
        console.log("Wallet created.");

        // 3. Discover globally
        console.log("Searching for wallets with credential hash...");
        const discovered = await client.findAllAuthoritiesByCredentialId(credentialIdHash);
        
        console.log("Discovered authorities:", discovered);

        // 4. Assertions
        expect(discovered.length).toBeGreaterThanOrEqual(1);
        const found = discovered.find((d: any) => d.authority === authPda);
        expect(found).toBeDefined();
        expect(found?.wallet).toBe(walletPda);
        expect(found?.role).toBe(0); // Owner
        expect(found?.authorityType).toBe(1); // Secp256r1
    }, 60000);
});
