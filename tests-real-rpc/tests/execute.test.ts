
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    lamports,
    type TransactionSigner
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext, getSystemTransferIx } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}


describe("Instruction: Execute", () => {
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

        await processInstruction(context, client.createWallet({
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
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPda, 200_000_000n));
    });

    it("Success: Owner executes a transfer", async () => {
        const recipient = (await generateKeyPairSigner()).address;

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 1_000_000n)
            ],
            authorizerSigner: owner,
        });

        await processInstruction(context, executeIx, [owner]);

        const balance = await context.rpc.getBalance(recipient).send();
        expect(balance.value).toBe(1_000_000n);
    });

    it("Success: Spender executes a transfer", async () => {
        const spender = await generateKeyPairSigner();
        const spenderBytes = Uint8Array.from(getAddressEncoder().encode(spender.address));
        const [spenderPda] = await findAuthorityPda(walletPda, spenderBytes);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: spenderPda,
            authType: 0,
            newRole: 2, // Spender
            authPubkey: spenderBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        const recipient = (await generateKeyPairSigner()).address;

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: spenderPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 1_000_000n)
            ],
            authorizerSigner: spender,
        });

        await processInstruction(context, executeIx, [spender]);

        const balance = await context.rpc.getBalance(recipient).send();
        expect(balance.value).toBe(1_000_000n);
    });

    it("Success: Session key executes a transfer", async () => {
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        // Create a valid session (expires in the far future)
        await processInstruction(context, client.createSession({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: BigInt(2 ** 62), // Far future
            authorizerSigner: owner,
        }), [owner]);

        const recipient = (await generateKeyPairSigner()).address;
        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: sessionPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 1_000_000n)
            ],
            authorizerSigner: sessionKey,
        });

        await processInstruction(context, executeIx, [sessionKey]);

        const balance = await context.rpc.getBalance(recipient).send();
        expect(balance.value).toBe(1_000_000n);
    });

    it("Failure: Session expired", async () => {
        const sessionKey = await generateKeyPairSigner();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        // Create a session that is already expired (expires at slot 0)
        await processInstruction(context, client.createSession({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: Uint8Array.from(getAddressEncoder().encode(sessionKey.address)),
            expiresAt: 0n,
            authorizerSigner: owner,
        }), [owner]);

        const recipient = (await generateKeyPairSigner()).address;
        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: sessionPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 100n)
            ],
            authorizerSigner: sessionKey,
        });

        const result = await tryProcessInstruction(context, executeIx, [sessionKey]);
        // SessionExpired error code (0xbc1 = 3009)
        expect(result.result).toMatch(/3009|0xbc1|simulation failed/i);
    });

    it("Failure: Unauthorized signatory", async () => {
        const thief = await generateKeyPairSigner();
        const recipient = (await generateKeyPairSigner()).address;

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 100n)
            ],
            authorizerSigner: thief,
        });

        const result = await tryProcessInstruction(context, executeIx, [thief]);
        // Signature mismatch or unauthorized
        expect(result.result).toMatch(/signature|unauthorized|simulation failed/i);
    });

    // --- P1: Cross-Wallet Execute Attack ---

    it("Failure: Authority from Wallet A cannot execute on Wallet B's vault", async () => {
        // Create Wallet B
        const userSeedB = getRandomSeed();
        const [walletPdaB] = await findWalletPda(userSeedB);
        const [vaultPdaB] = await findVaultPda(walletPdaB);
        const ownerB = await generateKeyPairSigner();
        const ownerBBytes = Uint8Array.from(getAddressEncoder().encode(ownerB.address));
        const [ownerBAuthPda, ownerBBump] = await findAuthorityPda(walletPdaB, ownerBBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPdaB,
            vault: vaultPdaB,
            authority: ownerBAuthPda,
            userSeed: userSeedB,
            authType: 0,
            authBump: ownerBBump,
            authPubkey: ownerBBytes,
            credentialHash: new Uint8Array(32),
        }));

        // Fund Wallet B's vault
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPdaB, 100_000_000n));

        const recipient = (await generateKeyPairSigner()).address;

        // Wallet A's owner tries to execute on Wallet B
        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPdaB,         // Target: Wallet B
            authority: ownerAuthPda,     // Using Wallet A's owner auth
            vault: vaultPdaB,
            innerInstructions: [
                getSystemTransferIx(vaultPdaB, recipient, 1_000_000n)
            ],
            authorizerSigner: owner,     // Wallet A's owner signer
        });

        const result = await tryProcessInstruction(context, executeIx, [owner]);
        expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
    });

    // --- P1: Self-Reentrancy Protection (Issue #10) ---

    it("Failure: Execute rejects self-reentrancy (calling back into LazorKit)", async () => {
        const PROGRAM_ID = "2m47smrvCRpuqAyX2dLqPxpAC1658n1BAQga1wRCsQiT" as import("@solana/kit").Address;

        // Build an inner instruction that calls back into the LazorKit program
        const reentrancyIx = {
            programAddress: PROGRAM_ID,
            accounts: [
                { address: context.payer.address, role: 3 },
                { address: walletPda, role: 0 },
            ],
            data: new Uint8Array([0]) // CreateWallet discriminator (doesn't matter, should be rejected before parsing)
        };

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [reentrancyIx],
            authorizerSigner: owner,
        });

        const result = await tryProcessInstruction(context, executeIx, [owner]);
        // SelfReentrancyNotAllowed = 3013 = 0xbc5
        expect(result.result).toMatch(/3013|0xbc5|simulation failed/i);
    });

    // --- P3: Execute Instruction Gaps ---

    it("Success: Execute batch — multiple transfers in one execution", async () => {
        const recipient1 = (await generateKeyPairSigner()).address;
        const recipient2 = (await generateKeyPairSigner()).address;
        const recipient3 = (await generateKeyPairSigner()).address;

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient1, 1_000_000n),
                getSystemTransferIx(vaultPda, recipient2, 2_000_000n),
                getSystemTransferIx(vaultPda, recipient3, 3_000_000n),
            ],
            authorizerSigner: owner,
        });

        await processInstruction(context, executeIx, [owner]);

        const bal1 = await context.rpc.getBalance(recipient1).send();
        const bal2 = await context.rpc.getBalance(recipient2).send();
        const bal3 = await context.rpc.getBalance(recipient3).send();

        expect(bal1.value).toBe(1_000_000n);
        expect(bal2.value).toBe(2_000_000n);
        expect(bal3.value).toBe(3_000_000n);
    });

    it("Success: Execute with empty inner instructions", async () => {
        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [], // Empty batch
            authorizerSigner: owner,
        });

        // The transaction should succeed but do nothing
        await processInstruction(context, executeIx, [owner]);
    });

    it("Failure: Execute with wrong vault PDA", async () => {
        // Generate a random keypair to use as a fake vault
        const fakeVault = await generateKeyPairSigner();
        const recipient = (await generateKeyPairSigner()).address;

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: fakeVault.address, // Use fake vault
            innerInstructions: [
                getSystemTransferIx(fakeVault.address, recipient, 1_000_000n)
            ],
            authorizerSigner: owner,
        });

        const result = await tryProcessInstruction(context, executeIx, [owner, fakeVault]);
        // Vault PDA validation in execute.rs should throw InvalidSeeds
        expect(result.result).toMatch(/simulation failed|InvalidSeeds/i);
    });
});
