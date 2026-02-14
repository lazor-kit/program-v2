
import { describe, it, expect, beforeAll } from "vitest";
import {
    Address,
    AccountRole,
    address,
} from "@solana/kit";
import { start } from "solana-bankrun";
import { PublicKey, Keypair, SystemProgram, Transaction, TransactionInstruction } from "@solana/web3.js";
import { LazorClient, findWalletPda, findVaultPda, findAuthorityPda, findSessionPda, buildExecuteInstruction, packCompactInstructions } from "../src";
import * as path from "path";

const PROGRAM_ID_STR = "Btg4mLUdMd3ov8PBtmuuFMAimLAdXyew9XmsGtuY9VcP";
const PROGRAM_ID = new PublicKey(PROGRAM_ID_STR);

describe("SDK Full Integration (Real SVM)", () => {
    let context: any;
    let client: LazorClient;

    beforeAll(async () => {
        context = await start(
            [{ name: "lazorkit_program", programId: PROGRAM_ID }],
            []
        );
        client = new LazorClient({} as any);
    }, 60000);

    async function processTransaction(ixs: TransactionInstruction[], signers: Keypair[]) {
        const tx = new Transaction();
        tx.recentBlockhash = context.lastBlockhash;
        tx.feePayer = context.payer.publicKey;
        ixs.forEach(ix => tx.add(ix));
        tx.sign(context.payer, ...signers);

        const result = await context.banksClient.processTransaction(tx);
        if (result.result) {
            process.stdout.write("\n\n--- TRANSACTION FAILED ---\n");
            process.stdout.write(`Result: ${result.result}\n`);
            if (result.meta?.logMessages) {
                result.meta.logMessages.forEach(m => process.stdout.write(`${m}\n`));
            }
            process.stdout.write("---------------------------\n\n");
            throw new Error(`Transaction failed: ${result.result}`);
        }
        return result;
    }

    async function processInstruction(ix: any, signers: Keypair[] = [], extraAccounts: any[] = []) {
        const keys = [
            ...ix.accounts.map((a: any) => ({
                pubkey: new PublicKey(a.address),
                isSigner: !!(a.role & 0x02),
                isWritable: !!(a.role & 0x01),
            })),
            ...extraAccounts
        ];

        return await processTransaction([
            new TransactionInstruction({
                programId: PROGRAM_ID,
                keys,
                data: Buffer.from(ix.data),
            })
        ], signers);
    }

    it("Full Flow: Create -> Add -> Remove -> Execute", async () => {
        const payer = context.payer;
        const userSeed = new Uint8Array(32).fill(1);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        // 1. Create Wallet (Master Authority)
        const masterKey = Keypair.generate();
        const masterIdHash = masterKey.publicKey.toBytes();
        const [masterAuthPda, masterBump] = await findAuthorityPda(walletPda, masterIdHash);

        await processInstruction(client.createWallet({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: masterAuthPda,
            userSeed,
            authType: 0,
            authBump: masterBump,
            authPubkey: masterKey.publicKey.toBytes(),
            credentialHash: new Uint8Array(32).fill(0),
        }));
        console.log("✓ Wallet & Master created");

        // 2. Add a Spender Authority
        const spenderKey = Keypair.generate();
        const [spenderAuthPda] = await findAuthorityPda(walletPda, spenderKey.publicKey.toBytes());

        await processInstruction(client.addAuthority({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: masterAuthPda,
            newAuthority: spenderAuthPda,
            authType: 0,
            newRole: 2, // Spender
            authPubkey: spenderKey.publicKey.toBytes(),
            credentialHash: new Uint8Array(32).fill(0),
            authorizerSigner: { address: masterKey.publicKey.toBase58() as Address } as any,
        }), [masterKey]);
        console.log("✓ Spender added");

        // 3. Remove the Spender
        await processInstruction(client.removeAuthority({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: masterAuthPda,
            targetAuthority: spenderAuthPda,
            refundDestination: payer.publicKey.toBase58() as Address,
            authorizerSigner: { address: masterKey.publicKey.toBase58() as Address } as any,
        }), [masterKey]);
        console.log("✓ Spender removed");

        // 4. Batch Execution (2 Transfers)
        await processTransaction([
            SystemProgram.transfer({
                fromPubkey: payer.publicKey,
                toPubkey: new PublicKey(vaultPda),
                lamports: 1_000_000_000,
            })
        ], []);

        const recipient1 = Keypair.generate().publicKey;
        const recipient2 = Keypair.generate().publicKey;

        const innerIx1: any = {
            programAddress: address(SystemProgram.programId.toBase58()),
            accounts: [
                { address: address(vaultPda), role: AccountRole.WRITABLE },
                { address: address(recipient1.toBase58()), role: AccountRole.WRITABLE },
            ],
            data: Buffer.concat([Buffer.from([2, 0, 0, 0]), Buffer.from(new BigUint64Array([50_000_000n]).buffer)])
        };
        const innerIx2: any = {
            programAddress: address(SystemProgram.programId.toBase58()),
            accounts: [
                { address: address(vaultPda), role: AccountRole.WRITABLE },
                { address: address(recipient2.toBase58()), role: AccountRole.WRITABLE },
            ],
            data: Buffer.concat([Buffer.from([2, 0, 0, 0]), Buffer.from(new BigUint64Array([30_000_000n]).buffer)])
        };

        const executeIx = buildExecuteInstruction({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            authority: masterAuthPda,
            vault: vaultPda,
            innerInstructions: [innerIx1, innerIx2],
            authorizerSigner: { address: masterKey.publicKey.toBase58() as Address } as any,
        });

        // Map accounts manually for buildExecuteInstruction
        const extraKeys = [
            { pubkey: recipient1, isSigner: false, isWritable: true },
            { pubkey: recipient2, isSigner: false, isWritable: true },
            { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ];

        await processInstruction(executeIx, [masterKey], extraKeys);

        const bal1 = await context.banksClient.getAccount(recipient1);
        const bal2 = await context.banksClient.getAccount(recipient2);
        expect(BigInt(bal1!.lamports)).toBe(50_000_000n);
        expect(BigInt(bal2!.lamports)).toBe(30_000_000n);
        console.log("✓ Batch execution (2 transfers) successful");
    });

    it("Integration: Session Key Flow", async () => {
        const payer = context.payer;
        const userSeed = new Uint8Array(32).fill(2);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const masterKey = Keypair.generate();
        const [masterAuthPda, masterBump] = await findAuthorityPda(walletPda, masterKey.publicKey.toBytes());

        await processInstruction(client.createWallet({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: masterAuthPda,
            userSeed,
            authType: 0,
            authBump: masterBump,
            authPubkey: masterKey.publicKey.toBytes(),
            credentialHash: new Uint8Array(32).fill(0),
        }));

        const sessionKey = Keypair.generate();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.publicKey.toBase58() as Address);

        // Create Session
        await processInstruction(client.createSession({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: masterAuthPda,
            session: sessionPda,
            sessionKey: sessionKey.publicKey.toBytes(),
            expiresAt: BigInt(Date.now() + 100000),
            authorizerSigner: { address: masterKey.publicKey.toBase58() as Address } as any,
        }), [masterKey]);

        // Fund vault
        await processTransaction([
            SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: new PublicKey(vaultPda), lamports: 200_000_000 })
        ], []);

        const recipient = Keypair.generate().publicKey;
        const packed = packCompactInstructions([{
            programIdIndex: 6,
            accountIndexes: [3, 5],
            data: new Uint8Array(Buffer.concat([Buffer.from([2, 0, 0, 0]), Buffer.from(new BigUint64Array([100_000_000n]).buffer)]))
        }]);

        await processInstruction(client.execute({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            authority: sessionPda,
            vault: vaultPda,
            packedInstructions: packed,
            authorizerSigner: { address: sessionKey.publicKey.toBase58() as Address } as any,
        }), [sessionKey], [
            { pubkey: recipient, isSigner: false, isWritable: true },
            { pubkey: SystemProgram.programId, isSigner: false, isWritable: false }
        ]);

        const acc = await context.banksClient.getAccount(recipient);
        expect(BigInt(acc!.lamports)).toBe(100_000_000n);
        console.log("✓ Session Key lifecycle verified");
    });

    it("Integration: Transfer Ownership", async () => {
        const payer = context.payer;
        const userSeed = new Uint8Array(32).fill(3);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const currentOwner = Keypair.generate();
        const [currentAuthPda, currentBump] = await findAuthorityPda(walletPda, currentOwner.publicKey.toBytes());

        await processInstruction(client.createWallet({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: currentAuthPda,
            userSeed,
            authType: 0,
            authBump: currentBump,
            authPubkey: currentOwner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32).fill(0),
        }));

        const newOwner = Keypair.generate();
        const [newAuthPda] = await findAuthorityPda(walletPda, newOwner.publicKey.toBytes());

        // Transfer Ownership
        await processInstruction(client.transferOwnership({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            currentOwnerAuthority: currentAuthPda,
            newOwnerAuthority: newAuthPda,
            authType: 0,
            authPubkey: newOwner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32).fill(0),
            authorizerSigner: { address: currentOwner.publicKey.toBase58() as Address } as any,
        }), [currentOwner]);

        console.log("✓ Ownership transferred");

        // Verify new owner can manage (e.g. create session)
        const sessionKey = Keypair.generate();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.publicKey.toBase58() as Address);

        await processInstruction(client.createSession({
            payer: { address: payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: newAuthPda,
            session: sessionPda,
            sessionKey: sessionKey.publicKey.toBytes(),
            expiresAt: BigInt(Date.now() + 100000),
            authorizerSigner: { address: newOwner.publicKey.toBase58() as Address } as any,
        }), [newOwner]);

        console.log("✓ New owner verified by creating session");
    });
});
