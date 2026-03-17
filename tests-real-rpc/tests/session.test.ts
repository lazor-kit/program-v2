
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    type TransactionSigner
} from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, type TestContext, getSystemTransferIx, PROGRAM_ID_STR } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "@lazorkit/codama-client/src";

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
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPda, 500_000_000n));
    });

    it("Success: Owner creates a session key", async () => {
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        await processInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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

    it("Success: Execution using session key", async () => {
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);
        const [vaultPda] = await findVaultPda(walletPda);

        await processInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: owner,
        }), [owner]);

        const recipient = (await generateKeyPairSigner()).address;
        const executeIx = client.buildExecute({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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

    // --- P2: Session Permission Boundaries ---

    it("Failure: Spender cannot create session", async () => {
        const spender = await generateKeyPairSigner();
        const spenderBytes = Uint8Array.from(getAddressEncoder().encode(spender.address));
        const [spenderPda] = await findAuthorityPda(walletPda, spenderBytes);

        // Owner adds a Spender
        await processInstruction(context, client.addAuthority({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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

        // Spender tries to create session → should fail
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        const result = await tryProcessInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: spenderPda, // Spender, not Admin/Owner
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: spender,
        }), [spender]);

        expect(result.result).toMatch(/0xbba|3002|simulation failed/i); // PermissionDenied
    });

    it("Failure: Session PDA cannot create another session", async () => {
        // Create a valid session first
        const sessionKey1 = await generateKeyPairSigner();
        const sessionKey1Bytes = Uint8Array.from(getAddressEncoder().encode(sessionKey1.address));
        const [sessionPda1] = await findSessionPda(walletPda, sessionKey1.address);

        await processInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda1,
            sessionKey: sessionKey1Bytes,
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: owner,
        }), [owner]);

        // Now try to use the session PDA as adminAuthority to create another session
        const sessionKey2 = await generateKeyPairSigner();
        const sessionKey2Bytes = Uint8Array.from(getAddressEncoder().encode(sessionKey2.address));
        const [sessionPda2] = await findSessionPda(walletPda, sessionKey2.address);

        const result = await tryProcessInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: sessionPda1, // Session PDA, not Authority
            session: sessionPda2,
            sessionKey: sessionKey2Bytes,
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: sessionKey1,
        }), [sessionKey1]);

        expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
    });

    // --- P4: Session Key Cannot Do Admin Actions ---

    it("Failure: Session key cannot add authority", async () => {
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        await processInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: owner,
        }), [owner]);

        // Try to use session PDA as adminAuthority to add authority
        const newUser = await generateKeyPairSigner();
        const newUserBytes = Uint8Array.from(getAddressEncoder().encode(newUser.address));
        const [newUserPda] = await findAuthorityPda(walletPda, newUserBytes);

        const result = await tryProcessInstruction(context, client.addAuthority({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: sessionPda, // Session PDA, not Authority
            newAuthority: newUserPda,
            authType: 0,
            newRole: 2,
            authPubkey: newUserBytes,
            credentialHash: new Uint8Array(32),
            authorizerSigner: sessionKey,
        }), [sessionKey]);

        expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
    });

    it("Failure: Session key cannot remove authority", async () => {
        // Create a spender first
        const spender = await generateKeyPairSigner();
        const spenderBytes = Uint8Array.from(getAddressEncoder().encode(spender.address));
        const [spenderPda] = await findAuthorityPda(walletPda, spenderBytes);

        await processInstruction(context, client.addAuthority({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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

        // Create a session
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        await processInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: BigInt(2 ** 62),
            authorizerSigner: owner,
        }), [owner]);

        // Try to use session PDA as adminAuthority to remove spender
        const result = await tryProcessInstruction(context, client.removeAuthority({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: sessionPda,
            targetAuthority: spenderPda,
            refundDestination: context.payer.address,
            authorizerSigner: sessionKey,
        }), [sessionKey]);

        expect(result.result).toMatch(/simulation failed|InvalidAccountData/i);
    });

    it("Success: Secp256r1 Admin creates a session", async () => {
        // Create Secp256r1 Admin
        const { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign, generateAuthenticatorData } = await import("./secp256r1Utils");
        const crypto = await import("crypto");
        const secpAdmin = await generateMockSecp256r1Signer();
        const [secpAdminPda] = await findAuthorityPda(walletPda, secpAdmin.credentialIdHash);

        await processInstruction(context, client.addAuthority({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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

        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        const expiresAt = 999999999n;

        const createSessionIx = client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: secpAdminPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt,
            // Since we're using Secp256r1, we don't pass an authorizerSigner.
        });

        // Append sysvars AFTER all existing accounts (config/treasury are consumed by iterator)
        createSessionIx.accounts = [
            ...(createSessionIx.accounts || []),
            { address: "Sysvar1nstructions1111111111111111111111111" as any, role: 0 },
            { address: "SysvarS1otHashes111111111111111111111111111" as any, role: 0 },
        ];

        // Fetch current slot and slotHash from SysvarS1otHashes
        const slotHashesAddress = "SysvarS1otHashes111111111111111111111111111" as Address;
        const accountInfo = await context.rpc.getAccountInfo(slotHashesAddress, { encoding: 'base64' }).send();
        const rawData = Buffer.from(accountInfo.value!.data[0] as string, 'base64');
        const currentSlot = new DataView(rawData.buffer, rawData.byteOffset, rawData.byteLength).getBigUint64(8, true);

        const sysvarIxIndex = createSessionIx.accounts.length - 2;      // Sysvar1nstructions position
        const sysvarSlotIndex = createSessionIx.accounts.length - 1; // SysvarSlotHashes position

        const authenticatorDataRaw = generateAuthenticatorData("example.com");
        const authPayload = buildSecp256r1AuthPayload(sysvarIxIndex, sysvarSlotIndex, authenticatorDataRaw, currentSlot);

        // The signed payload for CreateSession is `session_key` + `expires_at` + `payer`
        const signedPayload = new Uint8Array(32 + 8 + 32);
        signedPayload.set(sessionKeyBytes, 0);
        new DataView(signedPayload.buffer, signedPayload.byteOffset + 32).setBigUint64(0, expiresAt, true);
        signedPayload.set(new Uint8Array(getAddressEncoder().encode(context.payer.address)), 40);

        const currentSlotBytes = new Uint8Array(8);
        new DataView(currentSlotBytes.buffer).setBigUint64(0, currentSlot, true);

        const discriminator = new Uint8Array([5]); // CreateSession is 5
        const msgToSign = getSecp256r1MessageToSign(
            discriminator,
            authPayload,
            signedPayload,
            new Uint8Array(getAddressEncoder().encode(context.payer.address)),
            new Uint8Array(getAddressEncoder().encode(PROGRAM_ID_STR as import("@solana/kit").Address)),
            authenticatorDataRaw,
            currentSlotBytes
        );

        const sysvarIx = await createSecp256r1Instruction(secpAdmin, msgToSign);

        // Pack the payload into createSessionIx.data
        const originalData = createSessionIx.data;
        const finalCreateSessionData = new Uint8Array(originalData.length + authPayload.length);
        finalCreateSessionData.set(originalData, 0);
        finalCreateSessionData.set(authPayload, originalData.length);
        createSessionIx.data = finalCreateSessionData;

        const { tryProcessInstructions } = await import("./common");
        const result = await tryProcessInstructions(context, [sysvarIx, createSessionIx]);

        expect(result.result).toBe("ok");

        const sessionAcc = await client.getSession(sessionPda);
        expect(sessionAcc.discriminator).toBe(3); // Session
        expect(sessionAcc.sessionKey).toEqual(sessionKey.address);
    });
});
