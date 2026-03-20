import { expect, describe, it, beforeAll } from "vitest";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { setupTest, sendTx, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";
import { findVaultPda, findSessionPda, AuthType } from "@lazorkit/solita-client";

describe("Audit Regression Suite", () => {
    let ctx: TestContext;
    let walletPda: PublicKey;
    let vaultPda: PublicKey;
    let owner: Keypair;
    let ownerAuthPda: PublicKey;
    beforeAll(async () => {
        ctx = await setupTest();
        owner = Keypair.generate();

        // Create the wallet
        const { ix: ixCreate, walletPda: w, authorityPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: owner.publicKey,
        });
        await sendTx(ctx, [ixCreate]);

        walletPda = w;
        ownerAuthPda = authorityPda;
        const [v] = findVaultPda(walletPda);
        vaultPda = v;

        // Fund vault to simulate balances for executes
        await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, vaultPda, 100_000_000n)]);
    });

    it("Regression 1: SweepTreasury preserves rent-exemption and remains operational", async () => {
        const initialBalance = await ctx.connection.getBalance(ctx.treasuryShard);
        console.log(`Initial Shard Balance: ${initialBalance} lamports`);

        const pubkeyBytes = ctx.payer.publicKey.toBytes();
        const sum = pubkeyBytes.reduce((a: number, b: number) => a + b, 0);
        const shardId = sum % 16;

        const sweepIx = await ctx.highClient.sweepTreasury({
            admin: ctx.payer,
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
        const executeIx = await ctx.highClient.execute({
            payer: ctx.payer,
            walletPda,
            authorityPda: ownerAuthPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 890880n)
            ],
            signer: owner
        });

        await sendTx(ctx, [executeIx], [owner]);

        // Read action_fee
        const configInfo = await ctx.connection.getAccountInfo(ctx.configPda);
        const actionFee = configInfo!.data.readBigUInt64LE(48);

        const finalBalance = await ctx.connection.getBalance(ctx.treasuryShard);
        expect(finalBalance).toBe(RENT_EXEMPT_MIN + Number(actionFee));
    });

    it("Regression 2: CloseWallet rejects self-transfer to prevent burn", async () => {
        const closeIx = await ctx.highClient.closeWallet({
            payer: ctx.payer,
            walletPda,
            destination: vaultPda,
            adminType: AuthType.Ed25519,
            adminSigner: owner
        });

        const result = await tryProcessInstructions(ctx, [closeIx], [ctx.payer, owner]);
        expect(result.result).not.toBe("ok");
    });

    it("Regression 3: CloseSession rejects Config PDA spoofing", async () => {
        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

        const { ix: ixCreateSession } = await ctx.highClient.createSession({
            payer: ctx.payer,
            adminType: AuthType.Ed25519,
            adminSigner: owner,
            sessionKey: sessionKey.publicKey,
            expiresAt: BigInt(2 ** 62),
            walletPda
        });
        await sendTx(ctx, [ixCreateSession], [owner]);

        const [fakeConfigPda] = await PublicKey.findProgramAddress(
            [Buffer.from("fake_config")],
            ctx.payer.publicKey // random seed program
        );

        const closeSessionIx = await ctx.highClient.closeSession({
            payer: ctx.payer,
            walletPda,
            sessionPda: sessionPda,
            configPda: fakeConfigPda,
            authorizer: {
                authorizerPda: ownerAuthPda,
                signer: owner
            }
        });
        const result = await tryProcessInstructions(ctx, [closeSessionIx], [ctx.payer, owner]);
        expect(result.result).not.toBe("ok");
    });

    it("Regression 4: Verify no protocol fees on cleanup instructions", async () => {
        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletPda, sessionKey.publicKey);

        const { ix: ixCreateSession } = await ctx.highClient.createSession({
            payer: ctx.payer,
            adminType: AuthType.Ed25519,
            adminSigner: owner,
            sessionKey: sessionKey.publicKey,
            expiresAt: BigInt(2 ** 62),
            walletPda
        });
        await sendTx(ctx, [ixCreateSession], [owner]);

        const shardBalanceBefore = await ctx.connection.getBalance(ctx.treasuryShard);

        const closeSessionIx = await ctx.highClient.closeSession({
            payer: ctx.payer,
            walletPda,
            sessionPda: sessionPda,
            authorizer: {
                authorizerPda: ownerAuthPda,
                signer: owner
            }
        });
        await sendTx(ctx, [closeSessionIx], [owner]);

        const closeWalletIx = await ctx.highClient.closeWallet({
            payer: ctx.payer,
            walletPda,
            destination: ctx.payer.publicKey,
            adminType: AuthType.Ed25519,
            adminSigner: owner
        });
        await sendTx(ctx, [closeWalletIx], [owner]);

        const shardBalanceAfter = await ctx.connection.getBalance(ctx.treasuryShard);
        expect(shardBalanceAfter).toBe(shardBalanceBefore);
    });
});
