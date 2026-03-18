import { expect, describe, it, beforeAll } from "vitest";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { setupTest, sendTx, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "@lazorkit/solita-client";

function getRandomSeed() {
    const seed = new Uint8Array(32);
    crypto.getRandomValues(seed);
    return seed;
}

describe("Audit Regression Suite", () => {
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

        // Create the wallet
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

        // Fund vault to simulate balances for executes
        await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPda, 100_000_000n)]);
    });

    it("Regression 1: SweepTreasury preserves rent-exemption and remains operational", async () => {
        const initialBalance = await ctx.connection.getBalance(ctx.treasuryShard);
        console.log(`Initial Shard Balance: ${initialBalance} lamports`);

        const pubkeyBytes = ctx.payer.publicKey.toBytes();
        const sum = pubkeyBytes.reduce((a: number, b: number) => a + b, 0);
        const shardId = sum % 16;

        const sweepIx = ctx.client.sweepTreasury({
            admin: ctx.payer.publicKey,
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            destination: ctx.payer.publicKey,
            shardId,
        });

        const signature = await sendTx(ctx, [sweepIx]);

        const tx = await ctx.connection.getTransaction(signature, {
            maxSupportedTransactionVersion: 0
        });

        console.log("SweepTreasury Transaction Log:", tx?.meta?.logMessages);

        const postSweepBalance = await ctx.connection.getBalance(ctx.treasuryShard);
        const RENT_EXEMPT_MIN = 890_880; // for 0 bytes system account
        expect(postSweepBalance).toBe(RENT_EXEMPT_MIN);
        console.log(`Post-Sweep Shard Balance: ${postSweepBalance} lamports (Verified Rent-Exempt)`);

        // Operationality Check
        const recipient = Keypair.generate().publicKey;
        const executeIx = ctx.client.buildExecute({
            config: ctx.configPda,
            treasuryShard: ctx.treasuryShard,
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 890880n)
            ],
            authorizerSigner: owner.publicKey,
        });

        await sendTx(ctx, [executeIx], [owner]);

        // Read action_fee
        const configInfo = await ctx.connection.getAccountInfo(ctx.configPda);
        const actionFee = configInfo!.data.readBigUInt64LE(48);

        const finalBalance = await ctx.connection.getBalance(ctx.treasuryShard);
        expect(finalBalance).toBe(RENT_EXEMPT_MIN + Number(actionFee));
    });

    it("Regression 2: CloseWallet rejects self-transfer to prevent burn", async () => {
        const closeIx = ctx.client.closeWallet({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            destination: vaultPda, // ATTACK: Self-transfer
            ownerSigner: owner.publicKey,
        });
        
        closeIx.keys.push({ pubkey: SystemProgram.programId, isWritable: false, isSigner: false });

        const result = await tryProcessInstructions(ctx, [closeIx], [ctx.payer, owner]);
        expect(result.result).not.toBe("ok");
    });

    it("Regression 3: CloseSession rejects Config PDA spoofing", async () => {
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

        const [fakeConfigPda] = await PublicKey.findProgramAddress(
            [Buffer.from("fake_config")],
            ctx.payer.publicKey // random seed program
        );

        const closeSessionIx = ctx.client.closeSession({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            session: sessionPda,
            config: fakeConfigPda, // SPOOFED
            authorizer: ownerAuthPda,
            authorizerSigner: owner.publicKey,
        });

        const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer, owner]);
        expect(result.result).not.toBe("ok");
    });

    it("Regression 4: Verify no protocol fees on cleanup instructions", async () => {
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

        const shardBalanceBefore = await ctx.connection.getBalance(ctx.treasuryShard);

        const closeSessionIx = ctx.client.closeSession({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            session: sessionPda,
            config: ctx.configPda,
            authorizer: ownerAuthPda,
            authorizerSigner: owner.publicKey,
        });

        await sendTx(ctx, [closeSessionIx], [owner]);

        const closeWalletIx = ctx.client.closeWallet({
            payer: ctx.payer.publicKey,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            destination: ctx.payer.publicKey,
            ownerSigner: owner.publicKey,
        });
        closeWalletIx.keys.push({ pubkey: SystemProgram.programId, isWritable: false, isSigner: false });

        await sendTx(ctx, [closeWalletIx], [owner]);

        const shardBalanceAfter = await ctx.connection.getBalance(ctx.treasuryShard);
        expect(shardBalanceAfter).toBe(shardBalanceBefore);
    });
});
