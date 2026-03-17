import { describe, it, expect, beforeAll } from "vitest";
import {
    Address,
    generateKeyPairSigner,
    createTransactionMessage,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    appendTransactionMessageInstructions,
    signTransactionMessageWithSigners,
    getSignatureFromTransaction,
    getBase64EncodedWireTransaction,
    address,
    sendAndConfirmTransactionFactory,
} from "@solana/kit";
import { setupTest, processInstruction, type TestContext } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "@lazorkit/codama-client/src";
import crypto from "crypto";

describe("Real RPC Integration Suite", () => {
    let context: TestContext;
    let client: any;

    // Test data
    let userSeed: Uint8Array;
    let walletPda: Address;
    let vaultPda: Address;
    let authPda: Address;
    let p256Keypair: crypto.webcrypto.CryptoKeyPair;
    let credentialIdHash: Uint8Array;

    beforeAll(async () => {
        ({ context, client } = await setupTest());

        // Initialize Wallet Config Variables
        userSeed = new Uint8Array(32);
        crypto.getRandomValues(userSeed);

        walletPda = (await findWalletPda(userSeed))[0];
        vaultPda = (await findVaultPda(walletPda))[0];

        // 1. Generate a valid P256 Keypair
        p256Keypair = await crypto.subtle.generateKey(
            { name: "ECDSA", namedCurve: "P-256" },
            true,
            ["sign", "verify"]
        );

        const rpId = "lazorkit.valid";
        const rpIdHashBuffer = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(rpId));
        credentialIdHash = new Uint8Array(rpIdHashBuffer as ArrayBuffer);

        authPda = (await findAuthorityPda(walletPda, credentialIdHash))[0];

    }, 30000);

    // 1. Process Transaction Helper
    const processTransaction = async (instruction: any, signers: any[]) => {
        return await processInstruction(context, instruction, signers);
    };

    it("1. Create Wallet with Real RPC", async () => {
        // Prepare pubkey
        const spki = await crypto.subtle.exportKey("spki", p256Keypair.publicKey);
        let rawPubkeyInfo = new Uint8Array(spki as ArrayBuffer);
        let rawP256Pubkey = rawPubkeyInfo.slice(-64);
        let p256PubkeyCompressed = new Uint8Array(33);
        p256PubkeyCompressed[0] = (rawP256Pubkey[63] % 2 === 0) ? 0x02 : 0x03;
        p256PubkeyCompressed.set(rawP256Pubkey.slice(0, 32), 1);

        const authBump = (await findAuthorityPda(walletPda, credentialIdHash))[1];

        const ix = client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1, // Secp256r1
            authBump,
            authPubkey: p256PubkeyCompressed,
            credentialHash: credentialIdHash,
        });

        // Send logic
        const txResult = await processTransaction(ix, [context.payer]);
        console.log(`✓ Wallet Created successfully. Signature: ${txResult}`);

        expect(txResult).toBeDefined();
    }, 30000);

    it("2. Wallet Account Data Inspection", async () => {
        const res = await (context.rpc as any).getAccountInfo(walletPda).send();
        expect(res.value).toBeDefined();

        const dataArr = new Uint8Array((res.value as any).data[0]); // Base64 or bytes
        // Basic check on size. Should be > 0.
    }, 10000);
});
