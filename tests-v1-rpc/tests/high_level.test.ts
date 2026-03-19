import { expect, describe, it, beforeAll } from "vitest";
import { Keypair } from "@solana/web3.js";
import { setupTest, getSystemTransferIx, sendTx, type TestContext } from "./common";
import { LazorClient, findAuthorityPda, findVaultPda, AuthType, Role } from "@lazorkit/solita-client";

describe("High-Level Wrapper (LazorClient)", () => {
    let ctx: TestContext;
    beforeAll(async () => {
        ctx = await setupTest();
    });

    it("should create wallet and execute transaction with simplified APIs", async () => {
        const owner = Keypair.generate();
        
        // 1. Create Wallet
        const { ix, walletPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey
        });
        await sendTx(ctx, [ix]);
        expect(walletPda).toBeDefined();

        const [vaultPda] = findVaultPda(walletPda);

        // Fund Vault so it has lamports to transfer out
        const fundIx = getSystemTransferIx(ctx.payer.publicKey, vaultPda, 10_000_000n);
        await sendTx(ctx, [fundIx]);

        const recipient = Keypair.generate().publicKey;
        const [authorityPda] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

        // 2. Execute InnerInstruction
        const executeIx = await ctx.highClient.execute({
            payer: ctx.payer,
            walletPda,
            authorityPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 1_000_000n)
            ],
            signer: owner
        });
        await sendTx(ctx, [executeIx], [owner]);

        const bal = await ctx.connection.getBalance(recipient);
        expect(bal).toBe(1_000_000);
    });

    it("should add authority using high-level methods", async () => {
        const owner = Keypair.generate();
        
        // 1. Create Wallet
        const { ix, walletPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey
        });
        await sendTx(ctx, [ix]);

        const newAuthority = Keypair.generate();

        // 2. Add Authority
        const { ix: ixAdd } = await ctx.highClient.addAuthority({
            payer: ctx.payer,
            walletPda,
            adminType: AuthType.Ed25519,
            adminSigner: owner,
            newAuthorityPubkey: newAuthority.publicKey.toBytes(),
            authType: AuthType.Ed25519,
            role: Role.Admin,
        });
        await sendTx(ctx, [ixAdd], [owner]);

        const [newAuthPda] = findAuthorityPda(walletPda, newAuthority.publicKey.toBytes());
        const accInfo = await ctx.connection.getAccountInfo(newAuthPda);
        expect(accInfo).toBeDefined();
        // Discriminator check (Authority=2)
        expect(accInfo!.data[0]).toBe(2);
    });

    it("should create wallet and execute via Transaction Builders (...Txn)", async () => {
        const owner = Keypair.generate();
        
        // 1. Create Wallet Transaction
        const { transaction, walletPda, authorityPda } = await ctx.highClient.createWalletTxn({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey
        });
        // Simply send the transaction building outputs
        await sendTx(ctx, transaction.instructions); // ctx doesn't support sendTransaction easily, use sendTx
        expect(walletPda).toBeDefined();

        const [vaultPda] = findVaultPda(walletPda);
        const fundIx = getSystemTransferIx(ctx.payer.publicKey, vaultPda, 10_000_000n);
        await sendTx(ctx, [fundIx]);

        const recipient = Keypair.generate().publicKey;

        // 2. Execute Transaction
        const execTx = await ctx.highClient.executeTxn({
            payer: ctx.payer,
            walletPda,
            authorityPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 1_000_000n)
            ],
            signer: owner
        });
        await sendTx(ctx, execTx.instructions, [owner]);

        const bal = await ctx.connection.getBalance(recipient);
        expect(bal).toBe(1_000_000);
    });
});
