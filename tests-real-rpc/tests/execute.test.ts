
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
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
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPda, 200_000_000n));
    });

    it("Success: Owner executes a transfer", async () => {
        const recipient = (await generateKeyPairSigner()).address;

        const executeIx = client.buildExecute({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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

    it("Success: Secp256r1 Admin executes a transfer", async () => {
        // Create Secp256r1 Admin
        const { generateMockSecp256r1Signer, createSecp256r1Instruction, buildSecp256r1AuthPayload, getSecp256r1MessageToSign } = await import("./secp256r1Utils");
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

        // Secp256r1 Admin executes a transfer
        const recipient = (await generateKeyPairSigner()).address;
        const innerInstructions = [
            getSystemTransferIx(vaultPda, recipient, 2_000_000n)
        ];
        const executeIx = client.buildExecute({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
            payer: context.payer,
            wallet: walletPda,
            authority: secpAdminPda,
            vault: vaultPda,
            innerInstructions,
            // Since we're using Secp256r1, we don't pass an authorizerSigner.
            // We'll calculate the payload manually.
        });

        // SDK accounts array is typically frozen, we MUST reassign a new array!
        executeIx.accounts = [
            ...(executeIx.accounts || []),
            { address: "Sysvar1nstructions1111111111111111111111111" as any, role: 0 },
            { address: "SysvarS1otHashes111111111111111111111111111" as any, role: 0 }
        ];

        const argsDataExecute = executeIx.data.subarray(1); // after discriminator

        // Fetch current slot and slotHash from SysvarS1otHashes
        const slotHashesAddress = "SysvarS1otHashes111111111111111111111111111" as Address;
        const accountInfo = await context.rpc.getAccountInfo(slotHashesAddress, { encoding: 'base64' }).send();
        const rawData = Buffer.from(accountInfo.value!.data[0] as string, 'base64');

        // SlotHashes layout:
        // u64 len
        // SlotHash[0]: u64 slot, 32 bytes hash
        const currentSlot = new DataView(rawData.buffer, rawData.byteOffset, rawData.byteLength).getBigUint64(8, true);
        const currentSlotHash = new Uint8Array(rawData.buffer, rawData.byteOffset + 16, 32);

        // SYSVAR Indexes
        // Execute already has: Payer(0), Wallet(1), Auth(2), Vault(3), InnerAccs...
        const sysvarIxIndex = executeIx.accounts.length - 2;
        const sysvarSlotIndex = executeIx.accounts.length - 1;

        const { generateAuthenticatorData } = await import("./secp256r1Utils");
        const authenticatorDataRaw = generateAuthenticatorData("example.com");

        // Build mock WebAuthn metadata payload
        const authPayload = buildSecp256r1AuthPayload(sysvarIxIndex, sysvarSlotIndex, authenticatorDataRaw, currentSlot);

        // Compute Accounts Hash (unique to Execute instruction binding)
        const systemProgramId = "11111111111111111111111111111111" as Address; // Transfer invokes System
        const accountsHashData = new Uint8Array(32 * 3);
        accountsHashData.set(getAddressEncoder().encode(systemProgramId), 0);
        accountsHashData.set(getAddressEncoder().encode(vaultPda), 32);
        accountsHashData.set(getAddressEncoder().encode(recipient), 64);

        const crypto = await import("crypto");
        const accountsHashHasher = crypto.createHash('sha256');
        accountsHashHasher.update(accountsHashData);
        const accountsHash = new Uint8Array(accountsHashHasher.digest());

        // The signed payload for Execute is `compact_instructions` (argsDataExecute) + `accounts_hash`
        const signedPayload = new Uint8Array(argsDataExecute.length + 32);
        signedPayload.set(argsDataExecute, 0);
        signedPayload.set(accountsHash, argsDataExecute.length);

        const currentSlotBytes = new Uint8Array(8);
        new DataView(currentSlotBytes.buffer).setBigUint64(0, currentSlot, true);

        const discriminator = new Uint8Array([4]); // Execute is 4
        const msgToSign = getSecp256r1MessageToSign(
            discriminator,
            authPayload,
            signedPayload,
            new Uint8Array(getAddressEncoder().encode(context.payer.address)),
            authenticatorDataRaw,
            currentSlotBytes
        );

        const sysvarIx = await createSecp256r1Instruction(secpAdmin, msgToSign);

        // Pack the payload into executeIx.data
        const finalExecuteData = new Uint8Array(1 + argsDataExecute.length + authPayload.length);
        finalExecuteData.set(discriminator, 0);
        finalExecuteData.set(argsDataExecute, 1);
        finalExecuteData.set(authPayload, 1 + argsDataExecute.length);
        executeIx.data = finalExecuteData;

        const { tryProcessInstructions } = await import("./common");
        const result = await tryProcessInstructions(context, [sysvarIx, executeIx]);

        expect(result.result).toBe("ok");

        const balance = await context.rpc.getBalance(recipient).send();
        expect(balance.value).toBe(2_000_000n);
    });

    it("Failure: Session expired", async () => {
        const sessionKey = await generateKeyPairSigner();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        // Create a session that is already expired (expires at slot 0)
        await processInstruction(context, client.createSession({
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
            config: context.configPda,
            treasuryShard: context.treasuryShard,
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
