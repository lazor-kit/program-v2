
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    type TransactionSigner
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("Instruction: ManageAuthority (Add/Remove)", () => {
    let context: TestContext;
    let client: any;
    let walletPda: Address;
    let owner: TransactionSigner;
    let ownerAuthPda: Address;

    beforeAll(async () => {
        ({ context, client } = await setupTest());

        // Setup a wallet
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

    it("Success: Owner adds an Admin", async () => {
        const newAdmin = await generateKeyPairSigner();
        const newAdminBytes = Uint8Array.from(getAddressEncoder().encode(newAdmin.address));
        const [newAdminPda] = await findAuthorityPda(walletPda, newAdminBytes);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: newAdminPda,
            authType: 0,
            newRole: 1, // Admin
            authPubkey: newAdminBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        const acc = await client.getAuthority(newAdminPda);
        expect(acc.role).toBe(1);
    });

    it("Success: Admin adds a Spender", async () => {
        // ... (existing Spender test)
        const spender = await generateKeyPairSigner();
        const spenderBytes = Uint8Array.from(getAddressEncoder().encode(spender.address));
        const [spenderPda] = await findAuthorityPda(walletPda, spenderBytes);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda, // Using owner to add admin for next step
            newAuthority: spenderPda,
            authType: 0,
            newRole: 2,
            authPubkey: spenderBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        const acc = await client.getAuthority(spenderPda);
        expect(acc.role).toBe(2);
    });

    it("Success: Owner adds a Secp256r1 Admin", async () => {
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

    it("Failure: Admin tries to add an Admin", async () => {
        const admin = await generateKeyPairSigner();
        const adminBytes = Uint8Array.from(getAddressEncoder().encode(admin.address));
        const [adminPda] = await findAuthorityPda(walletPda, adminBytes);
        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1,
            authPubkey: adminBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        const anotherAdmin = await generateKeyPairSigner();
        const anotherAdminBytes = Uint8Array.from(getAddressEncoder().encode(anotherAdmin.address));
        const [anotherAdminPda] = await findAuthorityPda(walletPda, anotherAdminBytes);

        const result = await tryProcessInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: adminPda,
            newAuthority: anotherAdminPda,
            authType: 0,
            newRole: 1, // Admin (Forbidden for Admin)
            authPubkey: anotherAdminBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: admin,
        }), [admin]);

        expect(result.result).toMatch(/0xbba|3002|simulation failed/i);
    });

    it("Success: Admin removes a Spender", async () => {
        const admin = await generateKeyPairSigner();
        const adminBytes = Uint8Array.from(getAddressEncoder().encode(admin.address));
        const [adminPda] = await findAuthorityPda(walletPda, adminBytes);
        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1,
            authPubkey: adminBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

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

        // Admin removes Spender
        await processInstruction(context, client.removeAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: adminPda,
            targetAuthority: spenderPda,
            refundDestination: context.payer.address,
            authorizerSigner: admin,
        }), [admin]);

        // Verify removed
        const { value: acc } = await context.rpc.getAccountInfo(spenderPda).send();
        expect(acc).toBeNull();
    });

    it("Failure: Spender tries to remove another Spender", async () => {
        const spender1 = await generateKeyPairSigner();
        const s1Bytes = Uint8Array.from(getAddressEncoder().encode(spender1.address));
        const [s1Pda] = await findAuthorityPda(walletPda, s1Bytes);
        const spender2 = await generateKeyPairSigner();
        const s2Bytes = Uint8Array.from(getAddressEncoder().encode(spender2.address));
        const [s2Pda] = await findAuthorityPda(walletPda, s2Bytes);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: s1Pda,
            authType: 0,
            newRole: 2,
            authPubkey: s1Bytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: s2Pda,
            authType: 0,
            newRole: 2,
            authPubkey: s2Bytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        const result = await tryProcessInstruction(context, client.removeAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: s1Pda,
            targetAuthority: s2Pda,
            refundDestination: context.payer.address,
            authorizerSigner: spender1,
        }), [spender1]);

        expect(result.result).toMatch(/0xbba|3002|simulation failed/i);
    });

    it("Success: Secp256r1 Admin removes a Spender", async () => {
        // Create Secp256r1 Admin
        const { generateMockSecp256r1Signer, createSecp256r1Instruction } = await import("./secp256r1Utils");
        const secpAdmin = await generateMockSecp256r1Signer();
        const [secpAdminPda] = await findAuthorityPda(walletPda, secpAdmin.credentialIdHash);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: secpAdminPda,
            authType: 1, // Secp256r1
            newRole: 1,  // Admin
            authPubkey: secpAdmin.publicKeyBytes,
            credentialHash: secpAdmin.credentialIdHash,
            authorizerSigner: owner,
        }), [owner]);

        // Create a disposable Spender via the Owner
        const victim = await generateKeyPairSigner();
        const victimBytes = Uint8Array.from(getAddressEncoder().encode(victim.address));
        const [victimPda] = await findAuthorityPda(walletPda, victimBytes);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: victimPda,
            authType: 0,
            newRole: 2,
            authPubkey: victimBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        // Secp256r1 Admin removes the victim
        const removeAuthIx = client.removeAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: secpAdminPda,
            targetAuthority: victimPda,
            refundDestination: context.payer.address,
        });

        // SDK doesn't automatically fetch Sysvar Instructions or SlotHashes for removeAuthority, we must add them for Secp256r1
        removeAuthIx.accounts = [
            ...(removeAuthIx.accounts || []),
            { address: "Sysvar1nstructions1111111111111111111111111" as any, role: 0 },
            { address: "SysvarS1otHashes111111111111111111111111111" as any, role: 0 }
        ];

        // Fetch current slot and slotHash from SysvarS1otHashes
        const slotHashesAddress = "SysvarS1otHashes111111111111111111111111111" as Address;
        const accountInfo = await context.rpc.getAccountInfo(slotHashesAddress, { encoding: 'base64' }).send();
        const rawData = Buffer.from(accountInfo.value!.data[0] as string, 'base64');
        const currentSlot = new DataView(rawData.buffer, rawData.byteOffset, rawData.byteLength).getBigUint64(8, true);
        const currentSlotHash = new Uint8Array(rawData.buffer, rawData.byteOffset + 16, 32);

        // SYSVAR Indexes
        const sysvarIxIndex = removeAuthIx.accounts.length - 2;
        const sysvarSlotIndex = removeAuthIx.accounts.length - 1;

        const { buildSecp256r1AuthPayload, getSecp256r1MessageToSign, generateAuthenticatorData } = await import("./secp256r1Utils");
        const authenticatorDataRaw = generateAuthenticatorData("example.com");

        const authPayload = buildSecp256r1AuthPayload(sysvarIxIndex, sysvarSlotIndex, authenticatorDataRaw, currentSlot);

        // The Secp256r1 signed payload for remove_authority is strictly `target_auth_pda` + `refund_dest`
        const signedPayload = new Uint8Array(64);
        signedPayload.set(getAddressEncoder().encode(victimPda), 0);
        signedPayload.set(getAddressEncoder().encode(context.payer.address), 32); // refund dest

        const currentSlotBytes = new Uint8Array(8);
        new DataView(currentSlotBytes.buffer).setBigUint64(0, currentSlot, true);

        const msgToSign = getSecp256r1MessageToSign(
            new Uint8Array([2]), // Discriminator for RemoveAuthority is 2
            authPayload,
            signedPayload,
            new Uint8Array(getAddressEncoder().encode(context.payer.address)),
            authenticatorDataRaw,
            currentSlotBytes
        );

        // Append authPayload to removeAuthIx data (since Secp256r1Authenticator parses it from instruction_data)
        const newIxData = new Uint8Array(removeAuthIx.data.length + authPayload.length);
        newIxData.set(removeAuthIx.data, 0);
        newIxData.set(authPayload, removeAuthIx.data.length);
        removeAuthIx.data = newIxData;

        const sysvarIx = await createSecp256r1Instruction(secpAdmin, msgToSign);

        const { tryProcessInstructions } = await import("./common");
        const result = await tryProcessInstructions(context, [sysvarIx, removeAuthIx]);

        expect(result.result).toBe("ok");

        // Verify removed
        const { value: acc } = await context.rpc.getAccountInfo(victimPda).send();
        expect(acc).toBeNull();
    });

    // --- Category 2: SDK Encoding Correctness ---

    it("Encoding: AddAuthority Secp256r1 data matches expected binary layout", async () => {
        const credentialIdHash = new Uint8Array(32).fill(0xCC);
        const p256Pubkey = new Uint8Array(33).fill(0xDD);
        p256Pubkey[0] = 0x03;

        const [newAuthPda] = await findAuthorityPda(walletPda, credentialIdHash);

        const ix = client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: newAuthPda,
            authType: 1, // Secp256r1
            newRole: 2,  // Spender
            authPubkey: p256Pubkey,
            credentialHash: credentialIdHash,
            authorizerSigner: owner,
        });

        const data = new Uint8Array(ix.data);
        // Layout: [disc(1)][authType(1)][newRole(1)][padding(6)][credIdHash(32)][pubkey(33)]
        // Total: 1 + 1 + 1 + 6 + 32 + 33 = 74
        expect(data[0]).toBe(1);                                                 // discriminator = AddAuthority
        expect(data[1]).toBe(1);                                                 // authType = Secp256r1
        expect(data[2]).toBe(2);                                                 // newRole = Spender
        expect(Uint8Array.from(data.subarray(9, 41))).toEqual(Uint8Array.from(credentialIdHash));     // credential_id_hash
        expect(Uint8Array.from(data.subarray(41, 74))).toEqual(Uint8Array.from(p256Pubkey));          // pubkey
    });

    // --- Category 4: RBAC Edge Cases ---

    it("Failure: Spender cannot add any authority", async () => {
        const spender = await generateKeyPairSigner();
        const spenderBytes = Uint8Array.from(getAddressEncoder().encode(spender.address));
        const [spenderPda] = await findAuthorityPda(walletPda, spenderBytes);

        // Owner adds a Spender
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

        // Spender tries to add another Spender → should fail
        const victim = await generateKeyPairSigner();
        const victimBytes = Uint8Array.from(getAddressEncoder().encode(victim.address));
        const [victimPda] = await findAuthorityPda(walletPda, victimBytes);

        const result = await tryProcessInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: spenderPda,
            newAuthority: victimPda,
            authType: 0,
            newRole: 2,
            authPubkey: victimBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: spender,
        }), [spender]);

        expect(result.result).toMatch(/0xbba|3002|simulation failed/i); // PermissionDenied
    });

    it("Failure: Admin cannot remove Owner", async () => {
        const admin = await generateKeyPairSigner();
        const adminBytes = Uint8Array.from(getAddressEncoder().encode(admin.address));
        const [adminPda] = await findAuthorityPda(walletPda, adminBytes);

        // Owner adds an Admin
        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1,
            authPubkey: adminBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        // Admin tries to remove Owner → should fail
        const result = await tryProcessInstruction(context, client.removeAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: adminPda,
            targetAuthority: ownerAuthPda,
            refundDestination: context.payer.address,
            authorizerSigner: admin,
        }), [admin]);

        expect(result.result).toMatch(/simulation failed|3002|0xbba/i); // PermissionDenied
    });

    // --- P1: Cross-Wallet Attack Tests ---

    it("Failure: Authority from Wallet A cannot add authority to Wallet B", async () => {
        // Create Wallet B with its own owner
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

        // Wallet A's owner tries to add authority to Wallet B
        const victim = await generateKeyPairSigner();
        const victimBytes = Uint8Array.from(getAddressEncoder().encode(victim.address));
        const [victimPda] = await findAuthorityPda(walletPdaB, victimBytes);

        const result = await tryProcessInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPdaB,         // Target: Wallet B
            adminAuthority: ownerAuthPda, // Using Wallet A's owner
            newAuthority: victimPda,
            authType: 0,
            newRole: 2,
            authPubkey: victimBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,     // Wallet A's owner signer
        }), [owner]);

        expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
    });

    it("Failure: Authority from Wallet A cannot remove authority in Wallet B", async () => {
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

        // Add a spender to Wallet B
        const spenderB = await generateKeyPairSigner();
        const spenderBBytes = Uint8Array.from(getAddressEncoder().encode(spenderB.address));
        const [spenderBPda] = await findAuthorityPda(walletPdaB, spenderBBytes);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPdaB,
            adminAuthority: ownerBAuthPda,
            newAuthority: spenderBPda,
            authType: 0,
            newRole: 2,
            authPubkey: spenderBBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: ownerB,
        }), [ownerB]);

        // Wallet A's owner tries to remove Wallet B's spender
        const result = await tryProcessInstruction(context, client.removeAuthority({
            payer: context.payer,
            wallet: walletPdaB,            // Target: Wallet B
            adminAuthority: ownerAuthPda,   // Using Wallet A's owner
            targetAuthority: spenderBPda,
            refundDestination: context.payer.address,
            authorizerSigner: owner,        // Wallet A's owner signer
        }), [owner]);

        expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
    });

    // --- P1: Duplicate Authority Creation ---

    it("Failure: Cannot add same authority twice", async () => {
        const newUser = await generateKeyPairSigner();
        const newUserBytes = Uint8Array.from(getAddressEncoder().encode(newUser.address));
        const [newUserPda] = await findAuthorityPda(walletPda, newUserBytes);

        // First add — should succeed
        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: newUserPda,
            authType: 0,
            newRole: 2,
            authPubkey: newUserBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        // Second add with same pubkey — should fail (AccountAlreadyInitialized)
        const result = await tryProcessInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: newUserPda,
            authType: 0,
            newRole: 2,
            authPubkey: newUserBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        expect(result.result).toMatch(/simulation failed|already in use|AccountAlreadyInitialized/i);
    });

    // --- P2: Owner Self-Removal Edge Case ---

    it("Edge: Owner can remove itself (leaves wallet ownerless)", async () => {
        // Create a fresh wallet for this test
        const userSeed = getRandomSeed();
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const o = await generateKeyPairSigner();
        const oBytes = Uint8Array.from(getAddressEncoder().encode(o.address));
        const [oPda, oBump] = await findAuthorityPda(wPda, oBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: wPda, vault: vPda, authority: oPda,
            userSeed, authType: 0, authBump: oBump,
            authPubkey: oBytes, credentialHash: new Uint8Array(32),
        }));

        // Owner removes itself
        await processInstruction(context, client.removeAuthority({
            payer: context.payer,
            wallet: wPda,
            adminAuthority: oPda,
            targetAuthority: oPda,
            refundDestination: context.payer.address,
            authorizerSigner: o,
        }), [o]);

        // Authority PDA should be closed
        const { value: acc } = await context.rpc.getAccountInfo(oPda).send();
        expect(acc).toBeNull();
    });
});
