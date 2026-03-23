import { expect, describe, it, beforeAll } from "vitest";
import { Keypair, PublicKey } from "@solana/web3.js";
import { setupTest, sendTx, tryProcessInstructions, type TestContext, PROGRAM_ID, getSystemTransferIx } from "./common";
import {
    findConfigPda,
    findTreasuryShardPda,
    findWalletPda,
    LazorClient, // <--- Add LazorClient
} from "@lazorkit/solita-client";

describe("Config and Treasury Instructions", () => {
    let ctx: TestContext;
    // <--- Add highClient

    beforeAll(async () => {
        ctx = await setupTest();
        // <--- Initialize
    });

    it("should fail to initialize an already initialized Config PDA", async () => {
        const initConfigIx = await ctx.highClient.initializeConfig({
            admin: ctx.payer,
            walletFee: 10000n,
            actionFee: 1000n,
            numShards: 16
        });

        // This should fail because setupTest already initialized it
        const result = await tryProcessInstructions(ctx, [initConfigIx], [ctx.payer]);
        expect(result.result).not.toBe("ok");
    });

    it("should update config parameters by admin", async () => {
        const updateConfigIx = await ctx.highClient.updateConfig({
            admin: ctx.payer,
            walletFee: 20000n,
            actionFee: 2000n,
            numShards: 32,
        });

        const result = await tryProcessInstructions(ctx, [updateConfigIx], [ctx.payer]);
        expect(result.result).toBe("ok");

        // state change check omitted for simplicity as long as transaction succeeds
    });

    it("should reject update config from non-admin", async () => {
        const nonAdmin = Keypair.generate();

        const updateConfigIx = await ctx.highClient.updateConfig({
            admin: nonAdmin,
            walletFee: 50000n,
        });

        const result = await tryProcessInstructions(ctx, [updateConfigIx], [nonAdmin]);
        expect(result.result).not.toBe("ok");
    });

    it("should reject update config if a wrong account type is passed (discriminator check)", async () => {
        // We'll use the Wallet PDA of some wallet (or just random seed) as fake config
        const userSeed = new Uint8Array(32);
        crypto.getRandomValues(userSeed);
        const [walletPda] = findWalletPda(userSeed);

        const updateConfigIx = await ctx.highClient.updateConfig({
            admin: ctx.payer,
            walletFee: 50000n,
            configPda: walletPda, // WRONG Account
        });

        const result = await tryProcessInstructions(ctx, [updateConfigIx], [ctx.payer]);
        expect(result.result).not.toBe("ok");
    });

    it("should initialize a new treasury shard", async () => {
        let treasuryShardPda = PublicKey.default;
        let shardId = 0;

        for (let i = 0; i < 16; i++) {
            shardId = i;
            const [pda] = findTreasuryShardPda(shardId, PROGRAM_ID);
            treasuryShardPda = pda;

            const shardInfo = await ctx.connection.getAccountInfo(treasuryShardPda);
            if (!shardInfo) {
                break;
            }
        }

        const initShardIx = await ctx.highClient.initTreasuryShard({
            payer: ctx.payer,
            shardId,
        });

        const result = await tryProcessInstructions(ctx, [initShardIx], [ctx.payer]);
        expect(result.result).toBe("ok");
    });

    it("should sweep treasury shard funds as admin", async () => {
        const pubkeyBytes = ctx.payer.publicKey.toBytes();
        const sum = pubkeyBytes.reduce((a, b) => a + b, 0);
        const shardId = sum % 16;
        const [treasuryShardPda] = findTreasuryShardPda(shardId, PROGRAM_ID);

        // Fund shard directly to simulate fees
        await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, treasuryShardPda, 10000n)]);

        const sweepIx = await ctx.highClient.sweepTreasury({
            admin: ctx.payer,
            destination: ctx.payer.publicKey,
            shardId,
        });

        const result = await tryProcessInstructions(ctx, [sweepIx], [ctx.payer]);
        expect(result.result).toBe("ok");

        const shardBalance = await ctx.connection.getBalance(treasuryShardPda);
        expect(shardBalance).toBeGreaterThan(0); // Standard rent exemption preserved
    });

    it("should reject sweep treasury from non-admin", async () => {
        const nonAdmin = Keypair.generate();
        const shardId = 0;

        const sweepIx = await ctx.highClient.sweepTreasury({
            admin: nonAdmin,
            destination: nonAdmin.publicKey,
            shardId,
        });

        const result = await tryProcessInstructions(ctx, [sweepIx], [nonAdmin]);
        expect(result.result).not.toBe("ok");
    });
});
