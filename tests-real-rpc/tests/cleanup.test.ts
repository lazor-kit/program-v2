import { expect, describe, it, beforeAll } from "vitest";
import {
    generateKeyPairSigner,
    lamports,
    getAddressEncoder,
    type Address,
} from "@solana/kit";
import { setupTest, processInstructions, tryProcessInstructions, PROGRAM_ID_STR } from "./common";
import {
    getCloseSessionInstruction,
    getCloseWalletInstruction,
    findWalletPda,
    findVaultPda,
    findAuthorityPda,
    findSessionPda
} from "@lazorkit/codama-client/src";

describe("Cleanup Instructions", () => {
    let context: any;
    let client: any;

    beforeAll(async () => {
        const setup = await setupTest();
        context = setup.context;
        client = setup.client;
    });

    it("should allow wallet owner to close an active session", async () => {
        const { payer, configPda } = context;

        // 1. Create a Wallet
        const owner = await generateKeyPairSigner();
        const userSeed = new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstructions(context, [client.createWallet({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        })], [payer]);

        // 2. Create a Session
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        // Expiration in the future (active)
        const validUntil = BigInt(Math.floor(Date.now() / 1000) + 3600);

        await processInstructions(context, [client.createSession({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: validUntil,
            authorizerSigner: owner,
        })], [payer, owner]);

        const closeSessionIx = getCloseSessionInstruction({
            payer: payer,
            wallet: walletPda,
            session: sessionPda,
            config: configPda,
            authorizer: ownerAuthPda,
            authorizerSigner: owner,
        });

        const result = await tryProcessInstructions(context, [closeSessionIx], [payer, owner]);
        expect(result.result).toBe("ok");
    });

    it("should allow contract admin to close an expired session", async () => {
        const { payer, configPda } = context;

        const owner = await generateKeyPairSigner();
        const userSeed = new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstructions(context, [client.createWallet({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        })], [payer]);

        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        // Expiration in the past (expired). We use 0 since the smart contract validates against the slot number.
        const validUntil = 0n;

        await processInstructions(context, [client.createSession({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: validUntil,
            authorizerSigner: owner,
        })], [payer, owner]);

        // Close the Session (Contract Admin acting as payer) without an authorizer
        const closeSessionIx = getCloseSessionInstruction({
            payer: payer, // Payer is the global config admin in this context
            wallet: walletPda,
            session: sessionPda,
            config: configPda,
        });

        const result = await tryProcessInstructions(context, [closeSessionIx], [payer]);
        expect(result.result).toBe("ok");
    });

    it("should reject contract admin closing an active session", async () => {
        const { payer, configPda } = context;

        const owner = await generateKeyPairSigner();
        const userSeed = new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstructions(context, [client.createWallet({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        })], [payer]);

        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);
        const validUntil = BigInt(Math.floor(Date.now() / 1000) + 3600); // active

        await processInstructions(context, [client.createSession({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: validUntil,
            authorizerSigner: owner,
        })], [payer, owner]);

        const closeSessionIx = getCloseSessionInstruction({
            payer: payer,
            wallet: walletPda,
            session: sessionPda,
            config: configPda,
        });

        const result = await tryProcessInstructions(context, [closeSessionIx], [payer]);
        // 0x1776 = InvalidAuthority (since admin isn't authorized for active session)
        expect(result.result).not.toBe("ok");
    });

    it("should reject a random user closing an expired session", async () => {
        const { payer, configPda } = context;

        const owner = await generateKeyPairSigner();
        const randomUser = await generateKeyPairSigner();
        const userSeed = new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstructions(context, [client.createWallet({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        })], [payer]);

        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);
        const validUntil = 0n; // expired

        await processInstructions(context, [client.createSession({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: validUntil,
            authorizerSigner: owner,
        })], [payer, owner]);

        const closeSessionIx = getCloseSessionInstruction({
            payer: randomUser,
            wallet: walletPda,
            session: sessionPda,
            config: configPda,
        });

        const result = await tryProcessInstructions(context, [closeSessionIx], [randomUser]);
        // Random user is not config admin and has no authorizer token
        expect(result.result).not.toBe("ok");
    });

    it("should allow wallet owner to close a wallet and sweep rent", async () => {
        const { rpc, payer, configPda } = context;

        const owner = await generateKeyPairSigner();
        const userSeed = new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstructions(context, [client.createWallet({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        })], [payer]);

        // Put some extra sol in the vault
        const systemTransferIx = {
            programAddress: "11111111111111111111111111111111" as Address,
            data: Uint8Array.from([2, 0, 0, 0, ...new Uint8Array(new BigUint64Array([25000000n]).buffer)]), // 0.025 SOL
            accounts: [
                { address: payer.address, role: 3, signer: payer },
                { address: vaultPda, role: 1 }
            ]
        };
        await processInstructions(context, [systemTransferIx], [payer]);

        const destWallet = await generateKeyPairSigner();

        const closeWalletIxRaw = getCloseWalletInstruction({
            payer: payer, // Transaction fee payer
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            ownerSigner: owner,
            destination: destWallet.address,
        });
        const closeWalletIx = {
            ...closeWalletIxRaw,
            accounts: [
                ...closeWalletIxRaw.accounts,
                { address: "11111111111111111111111111111111" as Address, role: 1 }
            ]
        };

        const result = await tryProcessInstructions(context, [closeWalletIx], [payer, owner]);
        expect(result.result).toBe("ok");

        const destBalance = await rpc.getBalance(destWallet.address).send();
        // Should have received vault funds + rent of wallet/vault
        expect(Number(destBalance.value)).toBeGreaterThan(25000000);
    });

    it("should reject non-owner from closing a wallet", async () => {
        const { payer, configPda } = context;

        const owner = await generateKeyPairSigner();
        const attacker = await generateKeyPairSigner();
        const userSeed = new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstructions(context, [client.createWallet({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        })], [payer]);

        const closeWalletIxRaw = getCloseWalletInstruction({
            payer: attacker,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            ownerSigner: attacker,
            destination: attacker.address,
        });
        const closeWalletIx = {
            ...closeWalletIxRaw,
            accounts: [
                ...closeWalletIxRaw.accounts,
                { address: "11111111111111111111111111111111" as Address, role: 1 }
            ]
        };

        const result = await tryProcessInstructions(context, [closeWalletIx], [attacker]);
        // 0x1776 = InvalidAuthority because the attacker's signer won't match the owner authority
        expect(result.result).not.toBe("ok");
    });

    it("should reject closing wallet if destination is the vault PDA", async () => {
        const { payer, configPda } = context;

        const owner = await generateKeyPairSigner();
        const userSeed = new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstructions(context, [client.createWallet({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        })], [payer]);

        const closeWalletIxRaw = getCloseWalletInstruction({
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            ownerSigner: owner,
            destination: vaultPda, // self-destruct bug reproduction
        });
        const closeWalletIx = {
            ...closeWalletIxRaw,
            accounts: [
                ...closeWalletIxRaw.accounts,
                { address: "11111111111111111111111111111111" as Address, role: 1 }
            ]
        };

        const result = await tryProcessInstructions(context, [closeWalletIx], [payer, owner]);
        // ProgramError::InvalidArgument = 160 = 0xa0
        expect(result.result).toMatch(/0xa0|160|InvalidArgument|invalid program argument/i);
    });

    it("should reject closing wallet if destination is the wallet PDA", async () => {
        const { payer, configPda } = context;

        const owner = await generateKeyPairSigner();
        const userSeed = new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

        await processInstructions(context, [client.createWallet({
            config: configPda,
            treasuryShard: context.treasuryShard,
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerBytes,
            credentialHash: new Uint8Array(32),
        })], [payer]);

        const closeWalletIxRaw = getCloseWalletInstruction({
            payer: payer,
            wallet: walletPda,
            vault: vaultPda,
            ownerAuthority: ownerAuthPda,
            ownerSigner: owner,
            destination: walletPda, // self-destruct bug reproduction
        });
        const closeWalletIx = {
            ...closeWalletIxRaw,
            accounts: [
                ...closeWalletIxRaw.accounts,
                { address: "11111111111111111111111111111111" as Address, role: 1 }
            ]
        };

        const result = await tryProcessInstructions(context, [closeWalletIx], [payer, owner]);
        // ProgramError::InvalidArgument = 160 = 0xa0
        expect(result.result).toMatch(/0xa0|160|InvalidArgument|invalid program argument/i);
    });
});
