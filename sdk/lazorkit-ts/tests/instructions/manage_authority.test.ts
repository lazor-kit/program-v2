
import { describe, it, expect, beforeAll } from "vitest";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { Address } from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../src";

describe("Instruction: ManageAuthority (Add/Remove)", () => {
    let context: any;
    let client: any;
    let walletPda: Address;
    let owner: Keypair;
    let ownerAuthPda: Address;

    beforeAll(async () => {
        ({ context, client } = await setupTest());

        // Setup a wallet
        const userSeed = new Uint8Array(32).fill(20);
        [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
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
    });

    it("Success: Owner adds an Admin", async () => {
        const newAdmin = Keypair.generate();
        const [newAdminPda] = await findAuthorityPda(walletPda, newAdmin.publicKey.toBytes());

        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: newAdminPda,
            authType: 0,
            newRole: 1, // Admin
            authPubkey: newAdmin.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        const acc = await client.getAuthority(newAdminPda);
        expect(acc.role).toBe(1);
    });

    it("Success: Admin adds a Spender", async () => {
        // Setup an Admin first
        const admin = Keypair.generate();
        const [adminPda] = await findAuthorityPda(walletPda, admin.publicKey.toBytes());
        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1,
            authPubkey: admin.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        // Admin adds Spender
        const spender = Keypair.generate();
        const [spenderPda] = await findAuthorityPda(walletPda, spender.publicKey.toBytes());

        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: adminPda,
            newAuthority: spenderPda,
            authType: 0,
            newRole: 2,
            authPubkey: spender.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: admin.publicKey.toBase58() as Address } as any,
        }), [admin]);

        const acc = await client.getAuthority(spenderPda);
        expect(acc.role).toBe(2);
    });

    it("Failure: Admin tries to add an Admin", async () => {
        const admin = Keypair.generate();
        const [adminPda] = await findAuthorityPda(walletPda, admin.publicKey.toBytes());
        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1,
            authPubkey: admin.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        const anotherAdmin = Keypair.generate();
        const [anotherAdminPda] = await findAuthorityPda(walletPda, anotherAdmin.publicKey.toBytes());

        const result = await tryProcessInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: adminPda,
            newAuthority: anotherAdminPda,
            authType: 0,
            newRole: 1, // Admin (Forbidden for Admin)
            authPubkey: anotherAdmin.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: admin.publicKey.toBase58() as Address } as any,
        }), [admin]);

        expect(result.result).toContain("custom program error: 0xbba");
    });

    it("Success: Admin removes a Spender", async () => {
        const admin = Keypair.generate();
        const [adminPda] = await findAuthorityPda(walletPda, admin.publicKey.toBytes());
        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1,
            authPubkey: admin.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

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

        // Admin removes Spender
        await processInstruction(context, client.removeAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: adminPda,
            targetAuthority: spenderPda,
            refundDestination: context.payer.publicKey.toBase58() as Address,
            authorizerSigner: { address: admin.publicKey.toBase58() as Address } as any,
        }), [admin]);

        // Verify removed
        const acc = await context.banksClient.getAccount(new PublicKey(spenderPda));
        expect(acc).toBeNull();
    });

    it("Failure: Spender tries to remove another Spender", async () => {
        const spender1 = Keypair.generate();
        const [s1Pda] = await findAuthorityPda(walletPda, spender1.publicKey.toBytes());
        const spender2 = Keypair.generate();
        const [s2Pda] = await findAuthorityPda(walletPda, spender2.publicKey.toBytes());

        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: s1Pda,
            authType: 0,
            newRole: 2,
            authPubkey: spender1.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: s2Pda,
            authType: 0,
            newRole: 2,
            authPubkey: spender2.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        const result = await tryProcessInstruction(context, client.removeAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: s1Pda,
            targetAuthority: s2Pda,
            refundDestination: context.payer.publicKey.toBase58() as Address,
            authorizerSigner: { address: spender1.publicKey.toBase58() as Address } as any,
        }), [spender1]);

        expect(result.result).toContain("custom program error: 0xbba");
    });
});
