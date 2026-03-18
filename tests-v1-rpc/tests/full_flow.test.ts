import { describe, it, expect, beforeAll } from "vitest";
import { setupTest, sendTx, type TestContext } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "@lazorkit/solita-client";
import { Connection, PublicKey } from "@solana/web3.js";

describe("Real RPC Integration Suite — Full Flow", () => {
    let ctx: TestContext;

    // Test data
    let userSeed: Uint8Array;
    let walletPda: PublicKey;
    let vaultPda: PublicKey;
    let authPda: PublicKey;
    let p256Keypair: CryptoKeyPair;
    let credentialIdHash: Uint8Array;

    beforeAll(async () => {
        ctx = await setupTest();

        // Initialize Wallet Config Variables
        userSeed = new Uint8Array(32);
        crypto.getRandomValues(userSeed);

        const [wPda] = findWalletPda(userSeed);
        walletPda = wPda;
        const [vPda] = findVaultPda(walletPda);
        vaultPda = vPda;

        // 1. Generate a valid P256 Keypair
        p256Keypair = await crypto.subtle.generateKey(
            { name: "ECDSA", namedCurve: "P-256" },
            true,
            ["sign", "verify"]
        );

        const rpId = "lazorkit.valid";
        const rpIdHashBuffer = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(rpId));
        credentialIdHash = new Uint8Array(rpIdHashBuffer);

        const [oPda] = findAuthorityPda(walletPda, credentialIdHash);
        authPda = oPda;

    }, 30000);

    it("1. Create Wallet with Real RPC", async () => {
        // Prepare pubkey
        const spki = await crypto.subtle.exportKey("spki", p256Keypair.publicKey);
        let rawPubkeyInfo = new Uint8Array(spki);
        let rawP256Pubkey = rawPubkeyInfo.slice(-64);
        let p256PubkeyCompressed = new Uint8Array(33);
        p256PubkeyCompressed[0] = (rawP256Pubkey[63] % 2 === 0) ? 0x02 : 0x03;
        p256PubkeyCompressed.set(rawP256Pubkey.slice(0, 32), 1);

        const [_, authBump] = findAuthorityPda(walletPda, credentialIdHash);

        const ix = ctx.client.createWallet({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1, // Secp256r1
            authBump,
            authPubkey: p256PubkeyCompressed,
            credentialHash: credentialIdHash,
        });

        const txResult = await sendTx(ctx, [ix]);
        console.log(`✓ Wallet Created successfully. Signature: ${txResult}`);

        expect(txResult).toBeDefined();
    }, 30000);

    it("2. Wallet Account Data Inspection", async () => {
        const res = await ctx.connection.getAccountInfo(walletPda);
        expect(res).toBeDefined();
        expect(res!.data).toBeDefined();
    }, 10000);
});
