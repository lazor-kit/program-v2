
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    type TransactionSigner
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("Instruction: TransferOwnership", () => {
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

    it("Success: Owner transfers ownership to another key", async () => {
        const userSeed = getRandomSeed(); // Unique seed
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const o = await generateKeyPairSigner();
        const oBytes = Uint8Array.from(getAddressEncoder().encode(o.address));
        const [oPda, oBump] = await findAuthorityPda(wPda, oBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: wPda,
            vault: vPda,
            authority: oPda,
            userSeed,
            authType: 0,
            authBump: oBump,
            authPubkey: oBytes,
            credentialHash: new Uint8Array(32),
        }));

        const newOwner = await generateKeyPairSigner();
        const newOwnerBytes = Uint8Array.from(getAddressEncoder().encode(newOwner.address));
        const [newOwnerPda] = await findAuthorityPda(wPda, newOwnerBytes);

        await processInstruction(context, client.transferOwnership({
            payer: context.payer,
            wallet: wPda,
            currentOwnerAuthority: oPda,
            newOwnerAuthority: newOwnerPda,
            authType: 0,
            authPubkey: newOwnerBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: o,
        }), [o]);

        const acc = await client.getAuthority(newOwnerPda);
        expect(acc.role).toBe(0); // New Owner
    });

    it("Failure: Admin cannot transfer ownership", async () => {
        const userSeed = getRandomSeed(); // Unique seed
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const o = await generateKeyPairSigner();
        const oBytes = Uint8Array.from(getAddressEncoder().encode(o.address));
        const [oPda, oBump] = await findAuthorityPda(wPda, oBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: wPda,
            vault: vPda,
            authority: oPda,
            userSeed,
            authType: 0,
            authBump: oBump,
            authPubkey: oBytes,
            credentialHash: new Uint8Array(32),
        }));

        // Setup an Admin
        const admin = await generateKeyPairSigner();
        const adminBytes = Uint8Array.from(getAddressEncoder().encode(admin.address));
        const [adminPda] = await findAuthorityPda(wPda, adminBytes);
        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: wPda,
            adminAuthority: oPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1,
            authPubkey: adminBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: o,
        }), [o]);

        const someoneElse = await generateKeyPairSigner();
        const someoneElseBytes = Uint8Array.from(getAddressEncoder().encode(someoneElse.address));
        const [someonePda] = await findAuthorityPda(wPda, someoneElseBytes);

        const result = await tryProcessInstruction(context, client.transferOwnership({
            payer: context.payer,
            wallet: wPda,
            currentOwnerAuthority: adminPda,
            newOwnerAuthority: someonePda,
            authType: 0,
            authPubkey: someoneElseBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: admin,
        }), [admin]);

        expect(result.result).toMatch(/0xbba|3002|simulation failed/i);
    });
});
