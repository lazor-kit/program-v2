
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    type TransactionSigner,
    address,
    AccountRole
} from "@solana/kit";
import { setupTest, processInstruction, type TestContext, getSystemTransferIx } from "./common";
import {
    findWalletPda,
    findVaultPda,
    findAuthorityPda,
    findSessionPda,
    packCompactInstructions,
} from "../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("SDK Full Integration (Real RPC)", () => {
    let context: TestContext;
    let client: any;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    it("Full Flow: Create -> Add -> Remove -> Execute", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        // 1. Create Wallet (Master Authority)
        const masterKey = await generateKeyPairSigner();
        const masterBytes = Uint8Array.from(getAddressEncoder().encode(masterKey.address));
        const [masterAuthPda, masterBump] = await findAuthorityPda(walletPda, masterBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: masterAuthPda,
            userSeed,
            authType: 0,
            authBump: masterBump,
            authPubkey: masterBytes,
            credentialHash: new Uint8Array(32).fill(0),
        }));

        // 2. Add a Spender Authority
        const spenderKey = await generateKeyPairSigner();
        const spenderBytes = Uint8Array.from(getAddressEncoder().encode(spenderKey.address));
        const [spenderAuthPda] = await findAuthorityPda(walletPda, spenderBytes);

        await processInstruction(context, client.addAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: masterAuthPda,
            newAuthority: spenderAuthPda,
            authType: 0,
            newRole: 2, // Spender
            authPubkey: spenderBytes,
            credentialHash: new Uint8Array(32).fill(0),
            authorizerSigner: masterKey,
        }), [masterKey]);

        // 3. Remove the Spender
        await processInstruction(context, client.removeAuthority({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: masterAuthPda,
            targetAuthority: spenderAuthPda,
            refundDestination: context.payer.address,
            authorizerSigner: masterKey,
        }), [masterKey]);

        // 4. Batch Execution (2 Transfers)
        // Fund vault first
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPda, 1_000_000_000n));

        const recipient1 = (await generateKeyPairSigner()).address;
        const recipient2 = (await generateKeyPairSigner()).address;

        const innerIx1 = {
            programAddress: "11111111111111111111111111111111" as Address,
            accounts: [
                { address: vaultPda, role: AccountRole.WRITABLE },
                { address: recipient1, role: AccountRole.WRITABLE },
            ],
            data: new Uint8Array([2, 0, 0, 0, ...new Uint8Array(new BigUint64Array([50_000_000n]).buffer)])
        };
        const innerIx2 = {
            programAddress: "11111111111111111111111111111111" as Address,
            accounts: [
                { address: vaultPda, role: AccountRole.WRITABLE },
                { address: recipient2, role: AccountRole.WRITABLE },
            ],
            data: new Uint8Array([2, 0, 0, 0, ...new Uint8Array(new BigUint64Array([30_000_000n]).buffer)])
        };

        const executeIx = client.buildExecute({
            payer: context.payer,
            wallet: walletPda,
            authority: masterAuthPda,
            vault: vaultPda,
            innerInstructions: [innerIx1, innerIx2],
            authorizerSigner: masterKey,
        });

        await processInstruction(context, executeIx, [masterKey]);

        const { value: acc1 } = await context.rpc.getAccountInfo(recipient1).send();
        const { value: acc2 } = await context.rpc.getAccountInfo(recipient2).send();
        expect(acc1!.lamports).toBe(50_000_000n);
        expect(acc2!.lamports).toBe(30_000_000n);
    });

    it("Integration: Session Key Flow", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const masterKey = await generateKeyPairSigner();
        const masterBytes = Uint8Array.from(getAddressEncoder().encode(masterKey.address));
        const [masterAuthPda, masterBump] = await findAuthorityPda(walletPda, masterBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: masterAuthPda,
            userSeed,
            authType: 0,
            authBump: masterBump,
            authPubkey: masterBytes,
            credentialHash: new Uint8Array(32).fill(0),
        }));

        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        // Create Session
        await processInstruction(context, client.createSession({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: masterAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: BigInt(Date.now() + 100000),
            authorizerSigner: masterKey,
        }), [masterKey]);

        // Fund vault
        await processInstruction(context, getSystemTransferIx(context.payer, vaultPda, 200_000_000n));

        const recipient = (await generateKeyPairSigner()).address;

        // programIdIndex: 6 (System Program in execute allAccounts)
        // accountIndexes: [3, 5] (vault, recipient)
        const packed = packCompactInstructions([{
            programIdIndex: 6,
            accountIndexes: [3, 5],
            data: new Uint8Array([2, 0, 0, 0, ...new Uint8Array(new BigUint64Array([100_000_000n]).buffer)])
        }]);

        await processInstruction(context, client.execute({
            payer: context.payer,
            wallet: walletPda,
            authority: sessionPda,
            vault: vaultPda,
            packedInstructions: packed,
            authorizerSigner: sessionKey,
        }), [sessionKey], [
            { address: recipient, role: AccountRole.WRITABLE },
            { address: "11111111111111111111111111111111" as Address, role: AccountRole.READONLY }
        ]);

        const { value: acc } = await context.rpc.getAccountInfo(recipient).send();
        expect(acc!.lamports).toBe(100_000_000n);
    });

    it("Integration: Transfer Ownership", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const currentOwner = await generateKeyPairSigner();
        const currentBytes = Uint8Array.from(getAddressEncoder().encode(currentOwner.address));
        const [currentAuthPda, currentBump] = await findAuthorityPda(walletPda, currentBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: currentAuthPda,
            userSeed,
            authType: 0,
            authBump: currentBump,
            authPubkey: currentBytes,
            credentialHash: new Uint8Array(32).fill(0),
        }));

        const newOwner = await generateKeyPairSigner();
        const newOwnerBytes = Uint8Array.from(getAddressEncoder().encode(newOwner.address));
        const [newAuthPda] = await findAuthorityPda(walletPda, newOwnerBytes);

        // Transfer Ownership
        await processInstruction(context, client.transferOwnership({
            payer: context.payer,
            wallet: walletPda,
            currentOwnerAuthority: currentAuthPda,
            newOwnerAuthority: newAuthPda,
            authType: 0,
            authPubkey: newOwnerBytes,
            credentialHash: new Uint8Array(32).fill(0),
            authorizerSigner: currentOwner,
        }), [currentOwner]);

        // Verify new owner can manage (e.g. create session)
        const sessionKey = await generateKeyPairSigner();
        const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

        await processInstruction(context, client.createSession({
            payer: context.payer,
            wallet: walletPda,
            adminAuthority: newAuthPda,
            session: sessionPda,
            sessionKey: sessionKeyBytes,
            expiresAt: BigInt(Date.now() + 100000),
            authorizerSigner: newOwner,
        }), [newOwner]);
    });
});
