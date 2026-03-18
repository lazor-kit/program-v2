import { expect, describe, it, beforeAll } from "vitest";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { setupTest, sendTx, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";
import {
    findWalletPda,
    findVaultPda,
    findAuthorityPda,
    findSessionPda
} from "@lazorkit/solita-client";

describe("Cleanup Instructions", () => {
    let ctx: TestContext;

    beforeAll(async () => {
        ctx = await setupTest();
    });

    const getRandomSeed = () => {
        const seed = new Uint8Array(32);
        crypto.getRandomValues(seed);
        return seed;
    };

    it("should allow wallet owner to close an active session", async () => {
        const owner = Keypair.generate();
        const userSeed = getRandomSeed();
        const [walletPda] = findWalletPda(userSeed);
        const [vaultPda] = findVaultPda(walletPda);
        const [ownerAuthPda, authBump] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await sendTx(ctx, [ctx.client.createWallet({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
        })]);

        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);
        const validUntil = BigInt(Math.floor(Date.now() / 1000) + 3600); // active

        await sendTx(ctx, [ctx.client.createSession({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: Array.from(sessionKey.publicKey.toBytes()),
            expiresAt: validUntil,
            authorizerSigner: owner.publicKey,
        })], [owner]);

        const closeSessionIx = ctx.client.closeSession({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            session: sessionPda,
            config: ctx.configPda,
            authorizer: ownerAuthPda,
            authorizerSigner: owner.publicKey,
        });

        const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer, owner]);
        expect(result.result).toBe("ok");
    });

    it("should allow contract admin to close an expired session", async () => {
        const owner = Keypair.generate();
        const userSeed = getRandomSeed();
        const [walletPda] = findWalletPda(userSeed);
        const [vaultPda] = findVaultPda(walletPda);
        const [ownerAuthPda, authBump] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await sendTx(ctx, [ctx.client.createWallet({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
        })]);

        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);
        const validUntil = 0n; // expired

        await sendTx(ctx, [ctx.client.createSession({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: Array.from(sessionKey.publicKey.toBytes()),
            expiresAt: validUntil,
            authorizerSigner: owner.publicKey,
        })], [owner]);

        const closeSessionIx = ctx.client.closeSession({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            session: sessionPda,
            config: ctx.configPda,
        });

        const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer]);
        expect(result.result).toBe("ok");
    });

    it("should reject contract admin closing an active session", async () => {
        const owner = Keypair.generate();
        const userSeed = getRandomSeed();
        const [walletPda] = findWalletPda(userSeed);
        const [vaultPda] = findVaultPda(walletPda);
        const [ownerAuthPda, authBump] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await sendTx(ctx, [ctx.client.createWallet({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
        })]);

        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);
        const validUntil = BigInt(Math.floor(Date.now() / 1000) + 3600); // active

        await sendTx(ctx, [ctx.client.createSession({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: Array.from(sessionKey.publicKey.toBytes()),
            expiresAt: validUntil,
            authorizerSigner: owner.publicKey,
        })], [owner]);

        const closeSessionIx = ctx.client.closeSession({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            session: sessionPda,
            config: ctx.configPda,
        });

        const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer]);
        expect(result.result).not.toBe("ok");
    });

    it("should allow wallet owner to close a wallet and sweep rent", async () => {
        const owner = Keypair.generate();
        const userSeed = getRandomSeed();
        const [walletPda] = findWalletPda(userSeed);
        const [vaultPda] = findVaultPda(walletPda);
        const [ownerAuthPda, authBump] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await sendTx(ctx, [ctx.client.createWallet({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
        })]);

        // Place lamports to simulate direct fees or balance
        await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPda, 25000000n)]);

        const destWallet = Keypair.generate();

        const closeWalletIx = ctx.client.closeWallet({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            ownerSigner: owner.publicKey,
            destination: destWallet.publicKey,
        });
        closeWalletIx.keys.push({
            pubkey: SystemProgram.programId,
            isWritable: false,
            isSigner: false,
        });

        const result = await tryProcessInstructions(ctx, [closeWalletIx], [ctx.payer, owner]);
        expect(result.result).toBe("ok");

        const destBalance = await ctx.connection.getBalance(destWallet.publicKey);
        expect(destBalance).toBeGreaterThan(25000000);
    });

    it("should reject non-owner from closing a wallet", async () => {
        const owner = Keypair.generate();
        const attacker = Keypair.generate();
        const userSeed = getRandomSeed();
        const [walletPda] = findWalletPda(userSeed);
        const [vaultPda] = findVaultPda(walletPda);
        const [ownerAuthPda, authBump] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await sendTx(ctx, [ctx.client.createWallet({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
        })]);

        const destWallet = Keypair.generate();

        const closeWalletIx = ctx.client.closeWallet({
            payer: attacker.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            ownerSigner: attacker.publicKey,
            destination: destWallet.publicKey,
        });
        closeWalletIx.keys.push({
            pubkey: SystemProgram.programId,
            isWritable: false,
            isSigner: false,
        });

        const result = await tryProcessInstructions(ctx, [closeWalletIx], [attacker]);
        expect(result.result).not.toBe("ok");
    });

    it("should reject closing wallet if destination is the vault PDA", async () => {
        const owner = Keypair.generate();
        const userSeed = getRandomSeed();
        const [walletPda] = findWalletPda(userSeed);
        const [vaultPda] = findVaultPda(walletPda);
        const [ownerAuthPda, authBump] = findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await sendTx(ctx, [ctx.client.createWallet({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
        })]);

        const closeWalletIx = ctx.client.closeWallet({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            ownerSigner: owner.publicKey,
            destination: vaultPda, // self-destruct bug check
        });
        closeWalletIx.keys.push({
            pubkey: SystemProgram.programId,
            isWritable: false,
            isSigner: false,
        });

        const result = await tryProcessInstructions(ctx, [closeWalletIx], [ctx.payer, owner]);
        // Expect fail
        expect(result.result).not.toBe("ok");
    });
});
