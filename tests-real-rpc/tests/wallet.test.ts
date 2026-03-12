
import { describe, it, expect, beforeAll } from "vitest";
import {
    getAddressEncoder,
    generateKeyPairSigner,
    type Address,
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext, getSystemTransferIx, PROGRAM_ID_STR } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../sdk/lazorkit-ts/src";
import { LazorClient } from "../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("Wallet Lifecycle (Create, Discovery, Ownership)", () => {
    let context: TestContext;
    let client: LazorClient;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    // --- Create Wallet ---

    it("Success: Create wallet with Ed25519 owner", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        }));

        const authAcc = await client.getAuthority(authPda);
        expect(authAcc.authorityType).toBe(0); // Ed25519
        expect(authAcc.role).toBe(0); // Owner
    });

    it("Success: Create wallet with Secp256r1 (WebAuthn) owner", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const credentialIdHash = getRandomSeed();
        const p256Pubkey = new Uint8Array(33).map(() => Math.floor(Math.random() * 256));
        p256Pubkey[0] = 0x02;

        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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

        const authAcc = await client.getAuthority(authPda);
        expect(authAcc.authorityType).toBe(1); // Secp256r1
        expect(authAcc.role).toBe(0); // Owner
    });

    // --- Discovery ---

    it("Discovery: Ed25519 — pubkey → PDA → wallet", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        }));

        // Discover
        const discoveredAuth = await client.getAuthorityByPublicKey(walletPda, owner.address);
        expect(discoveredAuth).not.toBeNull();
        expect(discoveredAuth!.wallet).toBe(walletPda);
    });

    it("Discovery: Secp256r1 — credential_id → PDA → wallet", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const credIdHash = getRandomSeed();
        const [authPda, authBump] = await findAuthorityPda(walletPda, credIdHash);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1,
            authBump,
            authPubkey: new Uint8Array(33).fill(1),
            credentialHash: credIdHash,
        }));

        const discoveredAuth = await client.getAuthorityByCredentialId(walletPda, credIdHash);
        expect(discoveredAuth).not.toBeNull();
        expect(discoveredAuth!.wallet).toBe(walletPda);
    });

    // --- Transfer Ownership ---

    it("Success: Transfer ownership (Ed25519 -> Ed25519)", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const currentOwner = await generateKeyPairSigner();
        const currentOwnerBytes = Uint8Array.from(getAddressEncoder().encode(currentOwner.address));
        const [currentAuthPda, currentBump] = await findAuthorityPda(walletPda, currentOwnerBytes);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: currentAuthPda,
            userSeed,
            authType: 0,
            authBump: currentBump,
            authPubkey: currentOwnerBytes,
            credentialHash: new Uint8Array(32),
        }));

        const newOwner = await generateKeyPairSigner();
        const newOwnerBytes = Uint8Array.from(getAddressEncoder().encode(newOwner.address));
        const [newAuthPda] = await findAuthorityPda(walletPda, newOwnerBytes);

        await processInstruction(context, client.transferOwnership({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            currentOwnerAuthority: currentAuthPda,
            newOwnerAuthority: newAuthPda,
            authType: 0,
            authPubkey: newOwnerBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: currentOwner,
        }), [currentOwner]);

        const acc = await client.getAuthority(newAuthPda);
        expect(acc.role).toBe(0); // Owner
    });

    it("Failure: Admin cannot transfer ownership", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = await generateKeyPairSigner();
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, bump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump: bump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        }));

        // Add Admin
        const admin = await generateKeyPairSigner();
        const adminBytes = Uint8Array.from(getAddressEncoder().encode(admin.address));
        const [adminPda] = await findAuthorityPda(walletPda, adminBytes);
        await processInstruction(context, client.addAuthority({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1, // Admin
            authPubkey: adminBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: owner,
        }), [owner]);

        // Admin tries to transfer
        const result = await tryProcessInstruction(context, client.transferOwnership({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            currentOwnerAuthority: adminPda,
            newOwnerAuthority: adminPda, // irrelevant
            authType: 0,
            authPubkey: adminBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: admin,
        }), [admin]);

        expect(result.result).toMatch(/0xbba|3002|simulation failed/i);
    });

    // --- P1: Duplicate Wallet Creation ---

    it("Failure: Cannot create wallet with same seed twice", async () => {
        const userSeed = getRandomSeed();
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const o = await generateKeyPairSigner();
        const oBytes = Uint8Array.from(getAddressEncoder().encode(o.address));
        const [aPda, aBump] = await findAuthorityPda(wPda, oBytes);

        // First creation — should succeed
        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda, vault: vPda, authority: aPda,
            userSeed, authType: 0, authBump: aBump,
            authPubkey: oBytes, credentialHash: new Uint8Array(32),
        }));

        // Second creation with same seed — should fail
        const o2 = await generateKeyPairSigner();
        const o2Bytes = Uint8Array.from(getAddressEncoder().encode(o2.address));
        const [a2Pda, a2Bump] = await findAuthorityPda(wPda, o2Bytes);

        const result = await tryProcessInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda, vault: vPda, authority: a2Pda,
            userSeed, authType: 0, authBump: a2Bump,
            authPubkey: o2Bytes, credentialHash: new Uint8Array(32),
        }));

        expect(result.result).toMatch(/simulation failed|already in use|AccountAlreadyInitialized/i);
    });

    // --- P1: Zero-Address Transfer Ownership (Issue #15) ---

    it("Failure: Cannot transfer ownership to zero address", async () => {
        const userSeed = getRandomSeed();
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const o = await generateKeyPairSigner();
        const oBytes = Uint8Array.from(getAddressEncoder().encode(o.address));
        const [aPda, aBump] = await findAuthorityPda(wPda, oBytes);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda, vault: vPda, authority: aPda,
            userSeed, authType: 0, authBump: aBump,
            authPubkey: oBytes, credentialHash: new Uint8Array(32),
        }));

        // Attempt transfer with zero pubkey
        const zeroPubkey = new Uint8Array(32).fill(0);
        const [zeroPda] = await findAuthorityPda(wPda, zeroPubkey);

        const result = await tryProcessInstruction(context, client.transferOwnership({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda,
            currentOwnerAuthority: aPda,
            newOwnerAuthority: zeroPda,
            authType: 0,
            authPubkey: zeroPubkey,
            credentialHash: new Uint8Array(32),
            authorizerSigner: o,
        }), [o]);

        expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
    });

    // --- P4: Ownership Transfer Verification ---

    it("Success: After transfer ownership, old owner account is closed", async () => {
        const userSeed = getRandomSeed();
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const oldOwner = await generateKeyPairSigner();
        const oldBytes = Uint8Array.from(getAddressEncoder().encode(oldOwner.address));
        const [oldPda, oldBump] = await findAuthorityPda(wPda, oldBytes);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda, vault: vPda, authority: oldPda,
            userSeed, authType: 0, authBump: oldBump,
            authPubkey: oldBytes, credentialHash: new Uint8Array(32),
        }));

        const newOwner = await generateKeyPairSigner();
        const newBytes = Uint8Array.from(getAddressEncoder().encode(newOwner.address));
        const [newPda] = await findAuthorityPda(wPda, newBytes);

        await processInstruction(context, client.transferOwnership({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda,
            currentOwnerAuthority: oldPda,
            newOwnerAuthority: newPda,
            authType: 0,
            authPubkey: newBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: oldOwner,
        }), [oldOwner]);

        // Old owner PDA should be closed (zeroed / null)
        const { value: oldAcc } = await context.rpc.getAccountInfo(oldPda).send();
        expect(oldAcc).toBeNull();

        // New owner should exist with role 0
        const newAcc = await client.getAuthority(newPda);
        expect(newAcc.role).toBe(0);
    });

    it("Failure: Old owner cannot act after ownership transfer", async () => {
        const userSeed = getRandomSeed();
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const oldOwner = await generateKeyPairSigner();
        const oldBytes = Uint8Array.from(getAddressEncoder().encode(oldOwner.address));
        const [oldPda, oldBump] = await findAuthorityPda(wPda, oldBytes);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda, vault: vPda, authority: oldPda,
            userSeed, authType: 0, authBump: oldBump,
            authPubkey: oldBytes, credentialHash: new Uint8Array(32),
        }));

        const newOwner = await generateKeyPairSigner();
        const newBytes = Uint8Array.from(getAddressEncoder().encode(newOwner.address));
        const [newPda] = await findAuthorityPda(wPda, newBytes);

        await processInstruction(context, client.transferOwnership({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda,
            currentOwnerAuthority: oldPda,
            newOwnerAuthority: newPda,
            authType: 0,
            authPubkey: newBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: oldOwner,
        }), [oldOwner]);

        // Old owner tries to add authority — should fail (authority PDA closed/zeroed)
        const victim = await generateKeyPairSigner();
        const victimBytes = Uint8Array.from(getAddressEncoder().encode(victim.address));
        const [victimPda] = await findAuthorityPda(wPda, victimBytes);

        const result = await tryProcessInstruction(context, client.addAuthority({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: wPda,
            adminAuthority: oldPda,
            newAuthority: victimPda,
            authType: 0,
            newRole: 2,
            authPubkey: victimBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: oldOwner,
        }), [oldOwner]);

        expect(result.result).toMatch(/simulation failed|IllegalOwner|InvalidAccountData/i);
    });

    it("Success: Secp256r1 Owner transfers ownership to Ed25519", async () => {
        const { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign, generateAuthenticatorData } = await import("./secp256r1Utils");
        const crypto = await import("crypto");

        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        // 1. Create Wallet with Secp256r1 Owner
        const secpOwner = await generateMockSecp256r1Signer();
        const [secpOwnerPda, ownerBump] = await findAuthorityPda(walletPda, secpOwner.credentialIdHash);

        await processInstruction(context, client.createWallet({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: secpOwnerPda,
            userSeed,
            authType: 1, // Secp256r1
            authBump: ownerBump,
            authPubkey: secpOwner.publicKeyBytes,
            credentialHash: secpOwner.credentialIdHash,
        }));

        // 2. Prepare new Ed25519 Owner
        const newOwner = await generateKeyPairSigner();
        const newOwnerBytes = Uint8Array.from(getAddressEncoder().encode(newOwner.address));
        const [newAuthPda] = await findAuthorityPda(walletPda, newOwnerBytes);

        // 3. Perform Transfer
        const transferIx = client.transferOwnership({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            currentOwnerAuthority: secpOwnerPda,
            newOwnerAuthority: newAuthPda,
            authType: 0, // Transfer to Ed25519
            authPubkey: newOwnerBytes,
            credentialHash: new Uint8Array(32),
            // No authorizerSigner for Secp256r1
        });

        // Append sysvars AFTER all existing accounts (config/treasury consumed by iterator)
        transferIx.accounts = [
            ...(transferIx.accounts || []),
            { address: "Sysvar1nstructions1111111111111111111111111" as any, role: 0 },
            { address: "SysvarS1otHashes111111111111111111111111111" as any, role: 0 },
            { address: "SysvarRent111111111111111111111111111111111" as any, role: 0 },
        ];

        // Fetch current slot and slotHash from SysvarS1otHashes
        const slotHashesAddress = "SysvarS1otHashes111111111111111111111111111" as Address;
        const accountInfo = await context.rpc.getAccountInfo(slotHashesAddress, { encoding: 'base64' }).send();
        const rawData = Buffer.from(accountInfo.value!.data[0] as string, 'base64');
        const currentSlot = new DataView(rawData.buffer, rawData.byteOffset, rawData.byteLength).getBigUint64(8, true);

        const sysvarIxIndex = transferIx.accounts.length - 3;      // Sysvar1nstructions position
        const sysvarSlotIndex = transferIx.accounts.length - 2; // SysvarSlotHashes position

        const authenticatorDataRaw = generateAuthenticatorData("example.com");
        const authPayload = buildSecp256r1AuthPayload(sysvarIxIndex, sysvarSlotIndex, authenticatorDataRaw, currentSlot);

        // The signed payload for TransferOwnership is `auth_type(1)` + `full_auth_data(32 for Ed25519)` + `payer(32)`
        const signedPayload = new Uint8Array(1 + 32 + 32);
        signedPayload[0] = 0; // New type Ed25519
        signedPayload.set(newOwnerBytes, 1);
        signedPayload.set(new Uint8Array(getAddressEncoder().encode(context.payer.address)), 33);

        const currentSlotBytes = new Uint8Array(8);
        new DataView(currentSlotBytes.buffer).setBigUint64(0, currentSlot, true);

        const discriminator = new Uint8Array([3]); // TransferOwnership is 3
        const msgToSign = getSecp256r1MessageToSign(
            discriminator,
            authPayload,
            signedPayload,
            new Uint8Array(getAddressEncoder().encode(context.payer.address)),
            new Uint8Array(getAddressEncoder().encode(PROGRAM_ID_STR as import("@solana/kit").Address)),
            authenticatorDataRaw,
            currentSlotBytes
        );

        const sysvarIx = await createSecp256r1Instruction(secpOwner, msgToSign);

        // Pack the payload into transferIx.data
        const originalData = transferIx.data;
        const finalTransferData = new Uint8Array(originalData.length + authPayload.length);
        finalTransferData.set(originalData, 0);
        finalTransferData.set(authPayload, originalData.length);
        transferIx.data = finalTransferData;

        const { tryProcessInstructions } = await import("./common");
        const result = await tryProcessInstructions(context, [sysvarIx, transferIx]);

        expect(result.result).toBe("ok");

        const acc = await client.getAuthority(newAuthPda);
        expect(acc.role).toBe(0); // Owner
        expect(acc.authorityType).toBe(0); // Ed25519
    });
});

