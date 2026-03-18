import { expect, describe, it, beforeAll } from "vitest";
import { Keypair } from "@solana/web3.js";
import { setupTest, getSystemTransferIx, type TestContext } from "./common";
import { LazorClient, findAuthorityPda, findVaultPda } from "@lazorkit/solita-client";

describe("High-Level Wrapper (LazorClient)", () => {
    let ctx: TestContext;
    let highClient: LazorClient;

    beforeAll(async () => {
        ctx = await setupTest();
        highClient = new LazorClient(ctx.connection);
    });

    it("should create wallet and execute transaction with simplified APIs", async () => {
        const owner = Keypair.generate();
        
        // 1. Create Wallet
        const { walletPda } = await highClient.createWallet({
            payer: ctx.payer,
            authType: 0,
            owner: owner.publicKey
        });
        expect(walletPda).toBeDefined();

        const [vaultPda] = findVaultPda(walletPda);

        // Fund Vault so it has lamports to transfer out
        const fundIx = getSystemTransferIx(ctx.payer.publicKey, vaultPda, 10_000_000n);
        await highClient.sendTx([fundIx], [ctx.payer]);

        const recipient = Keypair.generate().publicKey;
        const [authorityPda] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

        // 2. Execute InnerInstruction
        const executeSignature = await highClient.execute({
            payer: ctx.payer,
            walletPda,
            authorityPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 1_000_000n)
            ],
            signer: owner
        });
        expect(executeSignature).toBeDefined();

        const bal = await ctx.connection.getBalance(recipient);
        expect(bal).toBe(1_000_000);
    });

    it("should add authority using high-level methods", async () => {
        const owner = Keypair.generate();
        
        // 1. Create Wallet
        const { walletPda } = await highClient.createWallet({
            payer: ctx.payer,
            authType: 0,
            owner: owner.publicKey
        });

        const newAuthority = Keypair.generate();

        // 2. Add Authority
        const signature = await highClient.addAuthority({
            payer: ctx.payer,
            walletPda,
            adminType: 0,
            adminSigner: owner,
            newAuthorityPubkey: newAuthority.publicKey.toBytes(),
            authType: 0, // Ed25519
            role: 1, // Admin
        });
        expect(signature).toBeDefined();

        const [newAuthPda] = findAuthorityPda(walletPda, newAuthority.publicKey.toBytes());
        const accInfo = await ctx.connection.getAccountInfo(newAuthPda);
        expect(accInfo).toBeDefined();
        // Discriminator check (Authority=2)
        expect(accInfo!.data[0]).toBe(2);
    });
});
