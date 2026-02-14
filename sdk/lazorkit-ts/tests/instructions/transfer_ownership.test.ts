
import { describe, it, expect, beforeAll } from "vitest";
import { PublicKey, Keypair } from "@solana/web3.js";
import { Address } from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../src";

describe("Instruction: TransferOwnership", () => {
    let context: any;
    let client: any;
    let walletPda: Address;
    let owner: Keypair;
    let ownerAuthPda: Address;

    beforeAll(async () => {
        ({ context, client } = await setupTest());

        const userSeed = new Uint8Array(32).fill(30);
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

    it("Success: Owner transfers ownership to another key", async () => {
        const userSeed = new Uint8Array(32).fill(31); // Unique seed
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const o = Keypair.generate();
        const [oPda, oBump] = await findAuthorityPda(wPda, o.publicKey.toBytes());

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: wPda,
            vault: vPda,
            authority: oPda,
            userSeed,
            authType: 0,
            authBump: oBump,
            authPubkey: o.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        }));

        const newOwner = Keypair.generate();
        const [newOwnerPda] = await findAuthorityPda(wPda, newOwner.publicKey.toBytes());

        await processInstruction(context, client.transferOwnership({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: wPda,
            currentOwnerAuthority: oPda,
            newOwnerAuthority: newOwnerPda,
            authType: 0,
            authPubkey: newOwner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: o.publicKey.toBase58() as Address } as any,
        }), [o]);

        const acc = await client.getAuthority(newOwnerPda);
        expect(acc.role).toBe(0); // New Owner
    });

    it("Failure: Admin cannot transfer ownership", async () => {
        const userSeed = new Uint8Array(32).fill(32); // Unique seed
        const [wPda] = await findWalletPda(userSeed);
        const [vPda] = await findVaultPda(wPda);
        const o = Keypair.generate();
        const [oPda, oBump] = await findAuthorityPda(wPda, o.publicKey.toBytes());

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: wPda,
            vault: vPda,
            authority: oPda,
            userSeed,
            authType: 0,
            authBump: oBump,
            authPubkey: o.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        }));

        // Setup an Admin
        const admin = Keypair.generate();
        const [adminPda] = await findAuthorityPda(wPda, admin.publicKey.toBytes());
        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: wPda,
            adminAuthority: oPda,
            newAuthority: adminPda,
            authType: 0,
            newRole: 1,
            authPubkey: admin.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: o.publicKey.toBase58() as Address } as any,
        }), [o]);

        const someoneElse = Keypair.generate();
        const [someonePda] = await findAuthorityPda(wPda, someoneElse.publicKey.toBytes());

        const result = await tryProcessInstruction(context, client.transferOwnership({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: wPda,
            currentOwnerAuthority: adminPda,
            newOwnerAuthority: someonePda,
            authType: 0,
            authPubkey: someoneElse.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: admin.publicKey.toBase58() as Address } as any,
        }), [admin]);

        expect(result.result).toContain("custom program error: 0xbba");
    });
});
