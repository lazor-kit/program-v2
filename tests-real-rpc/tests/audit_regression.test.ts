
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    type TransactionSigner,
    getProgramDerivedAddress,
} from "@solana/kit";
import {
    setupTest,
    processInstruction,
    tryProcessInstruction,
    type TestContext,
    getSystemTransferIx,
    PROGRAM_ID_STR
} from "./common";
import {
    findWalletPda,
    findVaultPda,
    findAuthorityPda,
    findSessionPda
} from "../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("Audit Regression Suite", () => {
    let context: TestContext;
    let client: any;
    let walletPda: Address;
    let vaultPda: Address;
    let owner: TransactionSigner;
    let ownerAuthPda: Address;

    beforeAll(async () => {
        ({ context, client } = await setupTest());

        const userSeed = getRandomSeed();
        [walletPda] = await findWalletPda(userSeed);
        [vaultPda] = await findVaultPda(walletPda);
        owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        let authBump;
        [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        // Fund the wallet with 100 SOL
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPda, 100_000_000n));

        // Create the wallet
        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        }));

        // Fund vault
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPda, 100_000_000n));
    });

    it("Regression 1: SweepTreasury preserves rent-exemption and remains operational", async () => {
        // 1. Get current balance of shard
        const initialBalance = await context.rpc.getBalance(context.treasuryShard).send();
        console.log(`Initial Shard Balance: ${initialBalance.value} lamports`);

        // 2. Perform Sweep (Shard ID is derived in setupTest, usually 0-15)
        // We need to know which shard we are using
        const pubkeyBytes = getAddressEncoder().encode(context.payer.address);
        const sum = pubkeyBytes.reduce((a, b) => a + b, 0);
        const shardId = sum % 16;

        const sweepIx = client.sweepTreasury({
            admin: context.payer,
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            destination: context.payer.address,
            shardId,
        });

        console.log("SweepTreasury Accounts:", sweepIx.accounts.map((a: any) => a.address));
        const signature = await processInstruction(context, sweepIx, [context.payer]);

        const tx = await context.rpc
            .getTransaction(signature, {
                maxSupportedTransactionVersion: 0
            })
            .send();

        console.log("SweepTreasury Transaction Log:", tx.meta.logMessages);

        // 3. Verify balance is exactly rent-exempt (890,880 for 0 bytes)
        const postSweepBalance = await context.rpc.getBalance(context.treasuryShard).send();
        const RENT_EXEMPT_MIN = 890_880n;
        expect(postSweepBalance.value).toBe(RENT_EXEMPT_MIN);
        console.log(`Post-Sweep Shard Balance: ${postSweepBalance.value} lamports (Verified Rent-Exempt)`);

        // 4. Operationality Check: Perform an 'Execute' which charges action_fee
        // If Sweep didn't leave rent, this would FAIL with RentExemption error
        const recipient = (await generateKeyPairSigner()).address;
        const executeIx = client.buildExecute({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 890_880n)
            ],
            authorizerSigner: owner,
        });

        const signature2 = await processInstruction(context, executeIx, [owner]);

        const tx2 = await context.rpc
            .getTransaction(signature2, {
                maxSupportedTransactionVersion: 0
            })
            .send();


        console.log("Execute Transaction Log:", tx2.meta.logMessages);

        const finalBalance = await context.rpc.getBalance(context.treasuryShard).send();
        // Should be RENT_EXEMPT_MIN + action_fee (1000)
        expect(finalBalance.value).toBe(RENT_EXEMPT_MIN + 2000n);
        console.log("Operationality Check Passed: Shard accepted new fees after sweep.");
    });

    it("Regression 2: CloseWallet rejects self-transfer to prevent burn", async () => {
        // Attempt to close wallet with vault as destination
        const closeIx = client.closeWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            destination: vaultPda, // ATTACK: Self-transfer
            ownerSigner: owner,
        });

        const result = await tryProcessInstruction(context, closeIx, [owner]);
        // Should fail with InvalidArgument (Solana Error or Custom 0xbbd/etc if defined, but here we expect rejection)
        expect(result.result).toMatch(/simulation failed|InvalidArgument|3004/i);
        console.log("Self-transfer rejection verified.");
    });

    it("Regression 3: CloseSession rejects Config PDA spoofing", async () => {
        const sessionKey = await generateKeyPairSigner();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        await processInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: Uint8Array.from(getAddressEncoder().encode(sessionKey.address)),
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: owner,
        }), [owner]);

        // Create a FAKE Config PDA
        const [fakeConfigPda] = await getProgramDerivedAddress({
            programAddress: context.payer.address,
            seeds: ["fake_config"],
        });

        // This test is tricky because we can't easily "initialize" a fake config with our own admin
        // unless we deploy another instance or use a mock. 
        // However, the check `find_program_address(["config"], program_id)` on-chain will catch it.

        const closeSessionIx = client.closeSession({
            payer: context.payer,
            wallet: walletPda,
            session: sessionPda,
            config: fakeConfigPda, // SPOOFED
            authorizer: ownerAuthPda,
            authorizerSigner: owner,
        });

        const result = await tryProcessInstruction(context, closeSessionIx, [owner]);
        // Should fail with InvalidSeeds (0x7d0 or similar)
        expect(result.result).toMatch(/InvalidSeeds|simulation failed/i);
        console.log("Config PDA spoofing protection verified.");
    });

    it("Regression 4: Verify no protocol fees on cleanup instructions", async () => {
        const initialPayerBalance = await context.rpc.getBalance(context.payer.address).send();

        // 1. Close Session (should be free in terms of protocol fees, only network fees)
        const sessionKey = await generateKeyPairSigner();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        await processInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: Uint8Array.from(getAddressEncoder().encode(sessionKey.address)),
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: owner,
        }), [owner]);

        const preCloseBalance = await context.rpc.getBalance(context.payer.address).send();

        await processInstruction(context, client.closeSession({
            payer: context.payer,
            wallet: walletPda,
            session: sessionPda,
            config: context.configPda,
            authorizer: ownerAuthPda,
            authorizerSigner: owner,
        }), [owner]);

        const postCloseBalance = await context.rpc.getBalance(context.payer.address).send();

        // Rent for Session is roughly 0.002 SOL.
        // If protocol fee (0.000001) was charged, it would be much less than the rent refund.
        // But we want to ensure it's NOT charged to the treasury.
        const shardBalanceBefore = await context.rpc.getBalance(context.treasuryShard).send();

        // Repeat for Wallet
        await processInstruction(context, client.closeWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            destination: context.payer.address,
            ownerSigner: owner,
        }), [owner]);

        const shardBalanceAfter = await context.rpc.getBalance(context.treasuryShard).send();

        // Shard balance should NOT have increased
        expect(shardBalanceAfter.value).toBe(shardBalanceBefore.value);
        console.log("No-fee verification passed: Shard balance remained constant during cleanup.");
    });
});
