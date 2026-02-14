
import { describe, it, expect, beforeAll } from "vitest";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { Address } from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction, processTransaction } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "../../src";

describe("Instruction: Execute", () => {
    let context: any;
    let client: any;
    let walletPda: Address;
    let vaultPda: Address;
    let owner: Keypair;
    let ownerAuthPda: Address;

    beforeAll(async () => {
        ({ context, client } = await setupTest());

        const userSeed = new Uint8Array(32).fill(50);
        [walletPda] = await findWalletPda(userSeed);
        [vaultPda] = await findVaultPda(walletPda);
        owner = Keypair.generate();
        let authBump;
        [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        }));

        // Fund vault
        await processTransaction(context, [
            SystemProgram.transfer({ fromPubkey: context.payer.publicKey, toPubkey: new PublicKey(vaultPda), lamports: 10 * LAMPORTS_PER_SOL })
        ], []);
    });

    it("Success: Owner executes a transfer", async () => {
        const recipient = Keypair.generate().publicKey;

        const transferIx = SystemProgram.transfer({
            fromPubkey: new PublicKey(vaultPda),
            toPubkey: recipient,
            lamports: LAMPORTS_PER_SOL,
        });

        const executeIx = client.buildExecute({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [{
                programAddress: SystemProgram.programId.toBase58() as Address,
                accounts: transferIx.keys.map(k => ({ address: k.pubkey.toBase58() as Address, role: k.isWritable ? (k.isSigner ? 3 : 1) : (k.isSigner ? 2 : 0) })),
                data: transferIx.data
            }],
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        });

        await processInstruction(context, executeIx, [owner]);

        const balance = await context.banksClient.getBalance(recipient);
        expect(Number(balance)).toBe(LAMPORTS_PER_SOL);
    });

    it("Success: Spender executes a transfer", async () => {
        const spender = Keypair.generate();
        const [spenderPda] = await findAuthorityPda(walletPda, spender.publicKey.toBytes());
        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: spenderPda,
            authType: 0,
            newRole: 2,
            authPubkey: spender.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        const recipient = Keypair.generate().publicKey;
        const transferIx = SystemProgram.transfer({
            fromPubkey: new PublicKey(vaultPda),
            toPubkey: recipient,
            lamports: LAMPORTS_PER_SOL,
        });

        const executeIx = client.buildExecute({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            authority: spenderPda,
            vault: vaultPda,
            innerInstructions: [{
                programAddress: SystemProgram.programId.toBase58() as Address,
                accounts: transferIx.keys.map(k => ({ address: k.pubkey.toBase58() as Address, role: k.isWritable ? (k.isSigner ? 3 : 1) : (k.isSigner ? 2 : 0) })),
                data: transferIx.data
            }],
            authorizerSigner: { address: spender.publicKey.toBase58() as Address } as any,
        });

        await processInstruction(context, executeIx, [spender]);

        const balance = await context.banksClient.getBalance(recipient);
        expect(Number(balance)).toBe(LAMPORTS_PER_SOL);
    });

    it("Failure: Session expired", async () => {
        const sessionKey = Keypair.generate();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.publicKey.toBase58() as Address);

        // Create a session that is already expired (expires at slot 0)
        await processInstruction(context, client.createSession({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKey.publicKey.toBytes(),
            expiresAt: 0n,
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        const recipient = Keypair.generate().publicKey;
        const executeIx = client.buildExecute({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            authority: sessionPda,
            vault: vaultPda,
            innerInstructions: [{
                programAddress: SystemProgram.programId.toBase58() as Address,
                accounts: [
                    { address: vaultPda, role: 1 },
                    { address: recipient.toBase58() as Address, role: 1 }
                ],
                data: SystemProgram.transfer({ fromPubkey: new PublicKey(vaultPda), toPubkey: recipient, lamports: 100 }).data
            }],
            authorizerSigner: { address: sessionKey.publicKey.toBase58() as Address } as any,
        });

        const result = await tryProcessInstruction(context, executeIx, [sessionKey]);
        expect(result.result).toContain("custom program error: 0xbc1");
    });

    it("Failure: Unauthorized signatory", async () => {
        const thief = Keypair.generate();
        const recipient = Keypair.generate().publicKey;

        const executeIx = client.buildExecute({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            authority: ownerAuthPda,
            vault: vaultPda,
            innerInstructions: [{
                programAddress: SystemProgram.programId.toBase58() as Address,
                accounts: [
                    { address: vaultPda, role: 1 },
                    { address: recipient.toBase58() as Address, role: 1 }
                ],
                data: SystemProgram.transfer({ fromPubkey: new PublicKey(vaultPda), toPubkey: recipient, lamports: 100 }).data
            }],
            authorizerSigner: { address: thief.publicKey.toBase58() as Address } as any,
        });

        const result = await tryProcessInstruction(context, executeIx, [thief]);
        // Ed25519 authentication will fail because the signature won't match the ownerAuthPda's stored key
        expect(result.result).toContain("missing required signature for instruction");
    });
});
