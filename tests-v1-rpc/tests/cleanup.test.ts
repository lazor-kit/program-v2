import { expect, describe, it, beforeAll } from "vitest";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { setupTest, sendTx, getRandomSeed, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";
import {
    findWalletPda,
    findVaultPda,
    findAuthorityPda,
    findSessionPda,
    LazorClient,
    AuthType, // <--- Add AuthType
    Role      // <--- Add Role
} from "@lazorkit/solita-client";

describe("Cleanup Instructions", () => {
    let ctx: TestContext;
    // <--- Add highClient

    beforeAll(async () => {
        ctx = await setupTest();
        // <--- Initialize
    });



    it("should allow wallet owner to close an active session", async () => {
        const owner = Keypair.generate();
        const { ix: ixCreate, walletPda, authorityPda: ownerAuthPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey
        });
        await sendTx(ctx, [ixCreate]);

        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);
        const validUntil = BigInt(Math.floor(Date.now() / 1000) + 3600); // active

        const { ix: ixCreateSession } = await ctx.highClient.createSession({
            payer: ctx.payer,
            adminType: AuthType.Ed25519,
            adminSigner: owner,
            sessionKey: sessionKey.publicKey,
            expiresAt: validUntil,
            walletPda
        });
        await sendTx(ctx, [ixCreateSession], [owner]);

        const closeSessionIx = await ctx.highClient.closeSession({
            payer: ctx.payer,
            walletPda,
            sessionPda: sessionPda,
            authorizer: {
                authorizerPda: ownerAuthPda,
                signer: owner
            }
        });

        const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer, owner]);
        expect(result.result).toBe("ok");
    });

    it("should allow contract admin to close an expired session", async () => {
        const owner = Keypair.generate();
        const { ix: ixCreate, walletPda, authorityPda: ownerAuthPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey
        });
        await sendTx(ctx, [ixCreate]);

        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);
        const validUntil = 0n; // expired

        const { ix: ixCreateSession } = await ctx.highClient.createSession({
            payer: ctx.payer,
            adminType: AuthType.Ed25519,
            adminSigner: owner,
            sessionKey: sessionKey.publicKey,
            expiresAt: validUntil,
            walletPda
        });
        await sendTx(ctx, [ixCreateSession], [owner]);

        const closeSessionIx = await ctx.highClient.closeSession({
            payer: ctx.payer,
            walletPda,
            sessionPda: sessionPda,
        });

        const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer]);
        expect(result.result).toBe("ok");
    });

    it("should reject contract admin closing an active session", async () => {
        const owner = Keypair.generate();
        const { ix: ixCreate, walletPda, authorityPda: ownerAuthPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey
        });
        await sendTx(ctx, [ixCreate]);

        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);
        const validUntil = BigInt(Math.floor(Date.now() / 1000) + 3600); // active

        const { ix: ixCreateSession } = await ctx.highClient.createSession({
            payer: ctx.payer,
            adminType: AuthType.Ed25519,
            adminSigner: owner,
            sessionKey: sessionKey.publicKey,
            expiresAt: validUntil,
            walletPda
        });
        await sendTx(ctx, [ixCreateSession], [owner]);

        const closeSessionIx = await ctx.highClient.closeSession({
            payer: ctx.payer,
            walletPda,
            sessionPda: sessionPda,
        });

        const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer]);
        expect(result.result).not.toBe("ok");
    });

    it("should allow wallet owner to close a wallet and sweep rent", async () => {
        const owner = Keypair.generate();
        const userSeed = getRandomSeed();

        const { ix, walletPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey,
            userSeed
        });
        await sendTx(ctx, [ix]);

        const [vaultPda] = findVaultPda(walletPda);

        // Place lamports to simulate direct fees or balance
        const [vPda] = findVaultPda(walletPda);
        await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vPda, 25000000n)]);

        const destWallet = Keypair.generate();

        const closeIx = await ctx.highClient.closeWallet({
            payer: ctx.payer,
            walletPda,
            destination: destWallet.publicKey,
            adminType: AuthType.Ed25519,
            adminSigner: owner
        });
        await sendTx(ctx, [closeIx], [owner]);

        const destBalance = await ctx.connection.getBalance(destWallet.publicKey);
        expect(destBalance).toBeGreaterThan(25000000);
    });

    it("should reject non-owner from closing a wallet", async () => {
        const owner = Keypair.generate();
        const attacker = Keypair.generate();
        const { ix: ixCreate, walletPda, authorityPda: ownerAuthPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey
        });
        await sendTx(ctx, [ixCreate]);

        const destWallet = Keypair.generate();

        const closeWalletIx = await ctx.highClient.closeWallet({
            payer: attacker,
            walletPda: walletPda,
            destination: destWallet.publicKey,
            adminType: AuthType.Ed25519,
            adminSigner: attacker,
            adminAuthorityPda: ownerAuthPda
        });

        const result = await tryProcessInstructions(ctx, [closeWalletIx], [attacker]);
        expect(result.result).not.toBe("ok");
    });

    it("should reject closing wallet if destination is the vault PDA", async () => {
        const owner = Keypair.generate();
        const { ix: ixCreate, walletPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey,
        });
        await sendTx(ctx, [ixCreate]);

        const [vaultPda] = findVaultPda(walletPda);

        const closeWalletIx = await ctx.highClient.closeWallet({
            payer: ctx.payer,
            walletPda: walletPda,
            destination: vaultPda,
            adminType: AuthType.Ed25519,
            adminSigner: owner
        });

        const result = await tryProcessInstructions(ctx, [closeWalletIx], [ctx.payer, owner]);
        // Expect fail
        expect(result.result).not.toBe("ok");
    });
});
