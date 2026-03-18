import { expect, describe, it, beforeAll } from "vitest";
import { Keypair, PublicKey } from "@solana/web3.js";
import { setupTest, sendTx, tryProcessInstructions, type TestContext } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "@lazorkit/solita-client";

function getRandomSeed() {
    const seed = new Uint8Array(32);
    crypto.getRandomValues(seed);
    return seed;
}

describe("Security Checklist Gaps", () => {
    let ctx: TestContext;
    let walletPda: PublicKey;
    let vaultPda: PublicKey;
    let owner: Keypair;
    let ownerAuthPda: PublicKey;

    beforeAll(async () => {
        ctx = await setupTest();

        const userSeed = getRandomSeed();
        const [w] = findWalletPda(userSeed);
        walletPda = w;
        const [v] = findVaultPda(walletPda);
        vaultPda = v;

        owner = Keypair.generate();
        const ownerBytes = owner.publicKey.toBytes();
        const [o, authBump] = findAuthorityPda(walletPda, ownerBytes);
        ownerAuthPda = o;

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
            authPubkey: ownerBytes,
        })]);
    }, 180_000);

    it("CreateSession rejects System Program spoofing", async () => {
        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

        const ix = ctx.client.createSession({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: Array.from(sessionKey.publicKey.toBytes()),
            expiresAt: 999999999n,
            authorizerSigner: owner.publicKey,
        });

        // Index 4 is SystemProgram
        const spoofedSystemProgram = Keypair.generate().publicKey;
        ix.keys = ix.keys.map((k: any, i: number) =>
            i === 4 ? { ...k, pubkey: spoofedSystemProgram } : k
        );

        const result = await tryProcessInstructions(ctx, [ix], [ctx.payer, owner]);
        expect(result.result).not.toBe("ok");
    });

    it("CloseSession: protocol admin cannot close an active session without wallet auth", async () => {
        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

        await sendTx(ctx, [ctx.client.createSession({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: Array.from(sessionKey.publicKey.toBytes()),
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: owner.publicKey,
        })], [owner]);

        // Call CloseSession without authorizer accounts
        const closeIx = ctx.client.closeSession({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            session: sessionPda,
            config: ctx.configPda,
        });

        const result = await tryProcessInstructions(ctx, [closeIx], [ctx.payer]);
        expect(result.result).not.toBe("ok");
    });
});
