
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    type TransactionSigner
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "../../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("Instruction: CreateSession", () => {
    let context: TestContext;
    let client: any;
    let walletPda: Address;
    let owner: TransactionSigner;
    let ownerAuthPda: Address;

    beforeAll(async () => {
        ({ context, client } = await setupTest());

        const userSeed = getRandomSeed();
        [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
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
    });

    it("Success: Owner creates a session key", async () => {
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        await processInstruction(context, client.createSession({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: 999999999n,
            authorizerSigner: owner,
        }), [owner]);

        const sessionAcc = await client.getSession(sessionPda);
        expect(sessionAcc.discriminator).toBe(3); // Session
        expect(sessionAcc.sessionKey).toEqual(sessionKey.address);
    });

    it("Failure: Spender cannot create a session key", async () => {
        const spender = await generateKeyPairSigner();
        const spenderBytes = Uint8Array.from(getAddressEncoder().encode(spender.address));
        const [spenderPda] = await findAuthorityPda(walletPda, spenderBytes);
        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: spenderPda,
            authType: 0,
            newRole: 2,
            authPubkey: spenderBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        const result = await tryProcessInstruction(context, client.createSession({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: spenderPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: 999999999n,
            authorizerSigner: spender,
        }), [spender]);

        expect(result.result).toMatch(/0xbba|3002|simulation failed/i);
    });
});
