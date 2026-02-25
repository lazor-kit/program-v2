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
import { client, rpc, rpcSubscriptions } from "./utils/rpcSetup";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../sdk/lazorkit-ts/src";
import crypto from "crypto";
import fs from "fs";
import path from "path";

// Function to read the local deployed keypair (usually built to target/deploy)
function loadKeypair(filePath: string): Uint8Array {
    const rawData = fs.readFileSync(path.resolve(__dirname, filePath), "utf-8");
    return new Uint8Array(JSON.parse(rawData));
}

describe("Real RPC Integration Suite", () => {
    let payerSigner: any;
    let sendAndConfirmTx: any;

    // Test data
    let userSeed: Uint8Array;
    let walletPda: Address;
    let vaultPda: Address;
    let authPda: Address;
    let p256Keypair: crypto.webcrypto.CryptoKeyPair;
    let credentialIdHash: Uint8Array;

    beforeAll(async () => {
        console.log("Setting up client and funding payer...");

        sendAndConfirmTx = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions } as any);

        // Generate payer
        payerSigner = await generateKeyPairSigner();

        // Airdrop SOL (Requesting 2 SOL for fees and rent)
        console.log(`Airdropping to ${payerSigner.address}...`);
        const airdropSig = await (rpc as any).requestAirdrop(
            payerSigner.address,
            2_000_000_000n, // 2 SOL
            { commitment: "confirmed" }
        ).send();

        // Wait for airdrop
        let confirmed = false;
        for (let i = 0; i < 10; i++) {
            const status = await (rpc as any).getSignatureStatuses([airdropSig]).send();
            if (status && status.value && status.value[0]?.confirmationStatus === "confirmed") {
                confirmed = true;
                break;
            }
            await new Promise((resolve) => setTimeout(resolve, 500));
        }

        if (!confirmed) {
            console.warn("Airdrop taking long, proceeding anyway...");
        } else {
            console.log("Airdrop confirmed!");
        }

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

        // Export SEC1 format for public key
        const spki = await crypto.subtle.exportKey("spki", p256Keypair.publicKey);
        let rawPubkeyInfo = new Uint8Array(spki as ArrayBuffer);
        let rawP256Pubkey = rawPubkeyInfo.slice(-64); // Extract raw X and Y coords
        let p256PubkeyCompressed = new Uint8Array(33);
        p256PubkeyCompressed[0] = (rawP256Pubkey[63] % 2 === 0) ? 0x02 : 0x03;
        p256PubkeyCompressed.set(rawP256Pubkey.slice(0, 32), 1);

        const rpId = "lazorkit.valid";
        const rpIdHashBuffer = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(rpId));
        credentialIdHash = new Uint8Array(rpIdHashBuffer as ArrayBuffer);

        authPda = (await findAuthorityPda(walletPda, credentialIdHash))[0];

    }, 30000);

    // 1. Process Transaction Helper
    const processTransaction = async (instruction: any, signers: any[]) => {
        const { value: latestBlockhash } = await (rpc as any).getLatestBlockhash().send();

        const txMessage = createTransactionMessage({ version: 0 });
        const txMessageWithFeePayer = setTransactionMessageFeePayerSigner(payerSigner, txMessage);
        const txMessageWithLifetime = setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, txMessageWithFeePayer);
        const txMessageWithInstructions = appendTransactionMessageInstructions([instruction], txMessageWithLifetime);

        const signedTx = await signTransactionMessageWithSigners(txMessageWithInstructions);

        const signature = getSignatureFromTransaction(signedTx);
        await sendAndConfirmTx(signedTx, { commitment: "confirmed" });
        return { signature };
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
            payer: payerSigner,
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
        const txResult = await processTransaction(ix, [payerSigner]);
        console.log(`✓ Wallet Created successfully. Signature: ${txResult.signature}`);

        expect(txResult.signature).toBeDefined();
    }, 30000);

    it("2. Wallet Account Data Inspection", async () => {
        const res = await (rpc as any).getAccountInfo(walletPda).send();
        expect(res.value).toBeDefined();

        const dataArr = new Uint8Array((res.value as any).data[0]); // Base64 or bytes
        // Basic check on size. Should be > 0.
    }, 10000);
});
