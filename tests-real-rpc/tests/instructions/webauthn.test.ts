
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("WebAuthn (Secp256r1) Support", () => {
    let context: TestContext;
    let client: any;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    it("Success: Create wallet with Secp256r1 (WebAuthn) owner", async () => {
        const userSeed = getRandomSeed();
        [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        // Mock WebAuthn values
        const credentialIdHash = getRandomSeed();
        const p256Pubkey = new Uint8Array(33).map(() => Math.floor(Math.random() * 256)); // Compressed P-256 key
        p256Pubkey[0] = 0x03;

        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1, // Secp256r1
            authBump,
            authPubkey: p256Pubkey,
            credentialHash: credentialIdHash,
        }));

        // Verify state
        const authAcc = await client.getAuthority(authPda);
        expect(authAcc.discriminator).toBe(2); // Authority
        expect(authAcc.authorityType).toBe(1); // Secp256r1
        expect(authAcc.role).toBe(0); // Owner
    });

    let walletPda: Address;

    it("Success: Add a Secp256r1 authority using Ed25519 owner", async () => {
        // Setup wallet with Ed25519 owner
        const userSeed = getRandomSeed();
        [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

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

        // Add Secp256r1 Admin
        const credentialIdHash = getRandomSeed();
        const p256Pubkey = new Uint8Array(33).map(() => Math.floor(Math.random() * 256));
        p256Pubkey[0] = 0x02;
        const [newAdminPda] = await findAuthorityPda(walletPda, credentialIdHash);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: newAdminPda,
            authType: 1, // Secp256r1
            newRole: 1, // Admin
            authPubkey: p256Pubkey,
            credentialHash: credentialIdHash,
            authorizerSigner: owner,
        }), [owner]);

        const acc = await client.getAuthority(newAdminPda);
        expect(acc.authorityType).toBe(1);
        expect(acc.role).toBe(1);
    });

    it("Failure: Execute with Secp256r1 authority fails with invalid payload", async () => {
        const userSeed = getRandomSeed();
        [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const credentialIdHash = getRandomSeed();
        const p256Pubkey = new Uint8Array(33).map(() => Math.floor(Math.random() * 256));
        p256Pubkey[0] = 0x02;
        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        // Create wallet with Secp256r1 owner
        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1,
            authBump,
            authPubkey: p256Pubkey,
            credentialHash: credentialIdHash,
        }));

        // Try to execute with dummy signature/payload
        // Secp256r1 Authenticator expects at least 12 bytes of auth_payload
        const dummyAuthPayload = new Uint8Array(20).fill(0);

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: authPda,
            vault: vaultPda,
            innerInstructions: [],
            signature: dummyAuthPayload, // Passed as 'signature' which becomes authority_payload in Execute
        });

        const result = await tryProcessInstruction(context, executeIx);
        // Should fail because it can't find SlotHashes or Instructions sysvar in the expected indices, 
        // or signature verification fails.
        expect(result.result).toMatch(/Unsupported sysvar|signature|simulation failed/i);
    });
});
