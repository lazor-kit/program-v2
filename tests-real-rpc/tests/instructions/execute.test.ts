
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    lamports,
    type TransactionSigner
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext, getSystemTransferIx } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "../../../sdk/lazorkit-ts/src";

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
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPda, 1_000_000_000n));
    });

    it("Success: Owner executes a transfer", async () => {
        const recipient = (await generateKeyPairSigner()).address;

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [
                getSystemTransferIx(vaultPda, recipient, 100_000_000n)
            ],
            authorizerSigner: owner,
        });

        await processInstruction(context, executeIx, [owner]);

        const balance = await context.rpc.getBalance(recipient).send();
        expect(balance.value).toBe(100_000_000n);
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
                getSystemTransferIx(vaultPda, recipient, 50_000_000n)
            ],
            authorizerSigner: spender,
        });

        await processInstruction(context, executeIx, [spender]);

        const balance = await context.rpc.getBalance(recipient).send();
        expect(balance.value).toBe(50_000_000n);
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
});
