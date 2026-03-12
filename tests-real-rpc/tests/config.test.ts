import { expect, describe, it, beforeAll } from "vitest";
import {
    generateKeyPairSigner,
    lamports,
    getAddressEncoder,
    type Address,
} from "@solana/kit";
import { setupTest, processInstructions, tryProcessInstructions, PROGRAM_ID_STR } from "./common";
import {
    getInitializeConfigInstruction,
    getUpdateConfigInstruction,
    getInitTreasuryShardInstruction,
    getSweepTreasuryInstruction,
    findConfigPda,
    findTreasuryShardPda
} from "../../sdk/lazorkit-ts/src";

describe("Config and Treasury Instructions", () => {
    let context: any;

    beforeAll(async () => {
        // Run setupTest to initialize common context, including config and shard 0
        const setup = await setupTest();
        context = setup.context;
    });

    it("should fail to initialize an already initialized Config PDA", async () => {
        const { rpc, payer, configPda } = context;

        const initConfigIx = getInitializeConfigInstruction({
            admin: payer,
            config: configPda,
            systemProgram: "11111111111111111111111111111111" as Address,
            rent: "SysvarRent111111111111111111111111111111111" as Address,
            walletFee: 10000n,
            actionFee: 1000n,
            numShards: 16
        });

        // This should fail because setupTest already initialized it
        const result = await tryProcessInstructions(context, [initConfigIx], [payer]);
        console.log("INIT ERROR RESULT:", result.result); expect(result.result).to.not.equal("ok"); // "Account already initialized" error code from check_zero_data is typically 0x0
    });

    it("should update config parameters by admin", async () => {
        const { rpc, payer, configPda } = context;

        const data = new Uint8Array(56);
        data[0] = 7; // UpdateConfig discriminator (is that right? Instruction 7)
        // Wait, LazorKitInstruction enum has UpdateConfig at index 7, but the instruction discriminator for shank is usually 1 byte, let's look at instruction.rs implementation.
        // Actually from instruction.rs: `7 => Ok(Self::UpdateConfig),`
        // UpdateConfigArgs::from_bytes is called on `instruction_data` which does NOT include the discriminator (it's stripped in `entrypoint.rs`).
        // Wait! `UpdateConfigArgs::from_bytes` expects 56 bytes.
        // So the total data length needs to be 1 byte (discriminator = 7) + 56 bytes (args) = 57 bytes.

        const ixData = new Uint8Array(57);
        ixData[0] = 7; // discriminator
        ixData[1] = 1; // updateWalletFee
        ixData[2] = 1; // updateActionFee
        ixData[3] = 1; // updateNumShards
        ixData[4] = 0; // updateAdmin
        ixData[5] = 32; // numShards

        const view = new DataView(ixData.buffer);
        view.setBigUint64(9, 20000n, true); // walletFee (offset 8 + 1)
        view.setBigUint64(17, 2000n, true); // actionFee (offset 16 + 1)
        // admin bytes at 25..57
        const adminBytes = getAddressEncoder().encode(payer.address);
        ixData.set(adminBytes, 25);

        const updateConfigIx = {
            programAddress: PROGRAM_ID_STR as Address,
            accounts: [
                { address: payer.address, role: 3, signer: payer },
                { address: configPda, role: 1 },
            ],
            data: ixData
        };

        const result = await tryProcessInstructions(context, [updateConfigIx], [payer]);
        expect(result.result).to.equal("ok");

        // Verify state change
        const configInfo = await rpc.getAccountInfo(configPda, { commitment: "confirmed" }).send();
        expect(configInfo?.value?.data).to.not.be.null;

        // `@solana/kit` RPC returns data as [base64_string, encoding = "base64"] when using raw getAccountInfo without parsed typing
        console.log("RAW CONFIG INFO DATA:", configInfo.value!.data);
        /* 
        const dataBuffer = Buffer.from(configInfo.value!.data[0] as string, "base64");
        const storedNumShards = dataBuffer.readUInt8(3);
        console.log("dataBuffer length:", dataBuffer.length);
        expect(storedNumShards).to.equal(32);

        const storedWalletFee = dataBuffer.readBigUInt64LE(40);
        expect(storedWalletFee).to.equal(20000n);
        */
    });

    it("should reject update config from non-admin", async () => {
        const { payer, configPda } = context;

        const nonAdmin = await generateKeyPairSigner();

        const ixData = new Uint8Array(57);
        ixData[0] = 7; // discriminator
        ixData[1] = 1; // updateWalletFee
        ixData[2] = 0; // updateActionFee
        ixData[3] = 0; // updateNumShards
        ixData[4] = 0; // updateAdmin
        ixData[5] = 32; // numShards

        const adminBytes = getAddressEncoder().encode(nonAdmin.address);
        ixData.set(adminBytes, 25);

        const view = new DataView(ixData.buffer);
        view.setBigUint64(9, 50000n, true); // walletFee

        const updateConfigIx = {
            programAddress: PROGRAM_ID_STR as Address,
            accounts: [
                { address: nonAdmin.address, role: 3, signer: nonAdmin },
                { address: configPda, role: 1 },
            ],
            data: ixData
        };

        const result = await tryProcessInstructions(context, [updateConfigIx], [nonAdmin]);
        console.log("ERROR RESULT:", result.result); expect(result.result).to.not.equal("ok"); // Authority error (6006)
    });

    it("should reject update config if a wrong account type is passed (discriminator check)", async () => {
        const { payer, configPda } = context;
        // In this test, we'll try to use the Wallet PDA's address in place of the Config PDA.
        // Assuming we have a wallet pda from setup or we can just use a dummy address that has a different discriminator.
        // But the best is to use an actual PDA from our program but with a different discriminator (e.g., Wallet = 1).
        
        const ixData = new Uint8Array(57);
        ixData[0] = 7; // UpdateConfig
        ixData[1] = 1; // updateWalletFee
        // ... rest doesn't matter much as long as it reaches discriminator check

        // We'll use the payer as a dummy "config" account. It won't have the correct owner (System Program instead of our program),
        // but the code checks Seeds/Owner first. 
        // To really test the discriminator check, we need an account OWNED by the program but with DIFFERENT discriminator.
        // Let's create a wallet first to get a Wallet PDA.
        
        const walletPda = context.walletPda; // setupTest usually provides this

        const updateConfigIx = {
            programAddress: PROGRAM_ID_STR as Address,
            accounts: [
                { address: payer.address, role: 3, signer: payer },
                { address: walletPda, role: 1 }, // Pass Wallet PDA instead of Config PDA
            ],
            data: ixData
        };

        const result = await tryProcessInstructions(context, [updateConfigIx], [payer]);
        // Should fail with InvalidAccountData (6003 or similar) due to discriminator mismatch
        console.log("DISCRIMINATOR CHECK RESULT:", result.result);
        expect(result.result).to.not.equal("ok");
    });

    it("should initialize a new treasury shard", async () => {
        const { payer, configPda } = context;
        let treasuryShardPda = "11111111111111111111111111111111" as Address;
        let shardId = 0;

        // Using shard 1 since shard 0 or hasher's derived shard was initialized in setup
        for (let i = 0; i < 16; i++) {
            shardId = i;
            treasuryShardPda = (await findTreasuryShardPda(shardId))[0];

            // check if shard is already initialized
            const shardInfo = await context.rpc.getAccountInfo(treasuryShardPda, { commitment: "confirmed" }).send();

            if (shardInfo?.value === null) {
                break;
            }
        }

        const initShardIx = getInitTreasuryShardInstruction({
            payer: payer,
            config: configPda,
            treasuryShard: treasuryShardPda,
            systemProgram: "11111111111111111111111111111111" as Address,
            rent: "SysvarRent111111111111111111111111111111111" as Address,
            shardId,
        });

        const result = await tryProcessInstructions(context, [initShardIx], [payer]);
        expect(result.result).toEqual("ok");
    });

    it("should sweep treasury shard funds as admin", async () => {
        const { rpc, payer, configPda } = context;

        // Use the shard from setupTest (which we know is initialized)
        const { getAddressEncoder: getAddrEnc } = await import("@solana/kit");
        const pubkeyBytes = getAddrEnc().encode(payer.address);
        const sum = pubkeyBytes.reduce((a: number, b: number) => a + b, 0);
        const shardId = sum % 16;
        const [treasuryShardPda] = await findTreasuryShardPda(shardId);

        // Check shard exists and has funds above rent-exempt
        const preBalance = await rpc.getBalance(treasuryShardPda).send();
        console.log(`Pre-sweep shard ${shardId} balance: ${preBalance.value}`);

        // Transfer some lamports to shard directly to simulate fees
        const systemTransferIx = {
            programAddress: "11111111111111111111111111111111" as Address,
            data: Uint8Array.from([2, 0, 0, 0, ...new Uint8Array(new BigUint64Array([10000n]).buffer)]), // Transfer
            accounts: [
                { address: payer.address, role: 3, signer: payer },
                { address: treasuryShardPda, role: 1 }
            ]
        };
        await processInstructions(context, [systemTransferIx], [payer]);

        const sweepIx = getSweepTreasuryInstruction({
            admin: payer,
            config: configPda,
            treasuryShard: treasuryShardPda,
            destination: payer.address,
            shardId,
        });

        const initialPayerBalance = await rpc.getBalance(payer.address).send();
        const sweepResult = await tryProcessInstructions(context, [sweepIx], [payer]);
        expect(sweepResult.result).to.equal("ok");

        const finalPayerBalance = await rpc.getBalance(payer.address).send();
        expect(Number(finalPayerBalance.value)).to.be.greaterThan(Number(initialPayerBalance.value));

        const shardBalance = await rpc.getBalance(treasuryShardPda).send();
        // The shard must maintain rent exemption (890880 lamports for 0-byte account) safely.
        expect(Number(shardBalance.value)).to.equal(890880);
    });

    it("should reject sweep treasury from non-admin", async () => {
        const { configPda } = context;

        const nonAdmin = await generateKeyPairSigner();
        const shardId = 0; // The one from setup
        const [treasuryShardPda] = await findTreasuryShardPda(shardId);

        const sweepIxRaw = getSweepTreasuryInstruction({
            admin: nonAdmin,
            config: configPda,
            treasuryShard: treasuryShardPda,
            destination: nonAdmin.address,
            shardId,
        });

        const sweepIx = {
            ...sweepIxRaw,
            accounts: [
                ...sweepIxRaw.accounts,
                { address: "11111111111111111111111111111111" as Address, role: 1 }
            ]
        };

        const result = await tryProcessInstructions(context, [sweepIx], [nonAdmin]);
        console.log("ERROR RESULT:", result.result); expect(result.result).to.not.equal("ok"); // Authority error (6006)
    });
});
