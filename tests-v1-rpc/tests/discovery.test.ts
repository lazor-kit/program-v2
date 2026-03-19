import { describe, it, expect, beforeAll } from "vitest";
import { Keypair, PublicKey } from "@solana/web3.js";
import { setupTest, sendTx, type TestContext, PROGRAM_ID } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda, LazorClient, AuthType } from "@lazorkit/solita-client";
import bs58 from "bs58";

async function findAllAuthoritiesByCredentialId(ctx: TestContext, credentialIdHash: Uint8Array): Promise<any[]> {
    const base58Hash = bs58.encode(Buffer.from(credentialIdHash));
    const accounts = await ctx.connection.getProgramAccounts(PROGRAM_ID, {
        filters: [
            { memcmp: { offset: 0, bytes: bs58.encode(Buffer.from([2])) } }, // Discriminator: Authority (2)
            { memcmp: { offset: 48, bytes: base58Hash } }                  // credentialIdHash starts at header offset (48)
        ]
    });

    return accounts.map((acc: any) => {
        const data = acc.account.data; // Buffer
        const role = data[2];
        const authorityType = data[1];
        const wallet = new PublicKey(data.subarray(16, 48));
        
        return {
            authority: acc.pubkey,
            wallet,
            role,
            authorityType
        };
    });
}

describe("Recovery by Credential Hash", () => {
    let ctx: TestContext;
    beforeAll(async () => {
        ctx = await setupTest();
        }, 30000);

    it("Should discover a wallet by its credential hash", async () => {
        // 1. Setup random data
        const userSeed = new Uint8Array(32);
        crypto.getRandomValues(userSeed);

        const credentialIdHash = new Uint8Array(32);
        crypto.getRandomValues(credentialIdHash);

        const [walletPda] = findWalletPda(userSeed);
        const [vaultPda] = findVaultPda(walletPda);
        const [authPda, authBump] = findAuthorityPda(walletPda, credentialIdHash);

        // Dummy Secp256r1 pubkey (33 bytes)
        const authPubkey = new Uint8Array(33).fill(7);

        // 2. Create the wallet
        console.log("Creating wallet for discovery test...");
        const { ix: createIx } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Secp256r1,
            pubkey: authPubkey,
            credentialHash: credentialIdHash,
            userSeed
        });

        await sendTx(ctx, [createIx]);
        console.log("Wallet created.");

        // 3. Discover globally
        console.log("Searching for wallets with credential hash...");
        const discovered = await findAllAuthoritiesByCredentialId(ctx, credentialIdHash);
        
        console.log("Discovered authorities:", discovered);

        // 4. Assertions
        expect(discovered.length).toBeGreaterThanOrEqual(1);
        const found = discovered.find((d: any) => d.authority.equals(authPda));
        expect(found).toBeDefined();
        expect(found?.wallet.equals(walletPda)).toBe(true);
        expect(found?.role).toBe(0); // Owner
        expect(found?.authorityType).toBe(1); // Secp256r1
    }, 60000);
});
