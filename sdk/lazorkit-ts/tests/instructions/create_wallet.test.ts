
import { describe, it, expect, beforeAll } from "vitest";
import { PublicKey, Keypair } from "@solana/web3.js";
import { Address } from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../src";
import { LazorClient } from "../../src";
import { ProgramTestContext } from "solana-bankrun";

describe("Instruction: CreateWallet", () => {
    let context: ProgramTestContext;
    let client: LazorClient;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    it("Success: Create wallet with Ed25519 owner", async () => {
        const userSeed = new Uint8Array(32).fill(10);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const owner = Keypair.generate();
        const [authPda, authBump] = await findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        }));

        // Verify state
        const walletAcc = await client.getWallet(walletPda);
        expect(walletAcc.discriminator).toBe(1); // Wallet
        expect(walletAcc.version).toBe(1);

        const authAcc = await client.getAuthority(authPda);
        expect(authAcc.discriminator).toBe(2); // Authority
        expect(authAcc.role).toBe(0); // Owner
        expect(authAcc.authorityType).toBe(0); // Ed25519
    });

    it("Failure: Account already initialized", async () => {
        const userSeed = new Uint8Array(32).fill(11);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = Keypair.generate();
        const [authPda, authBump] = await findAuthorityPda(walletPda, owner.publicKey.toBytes());

        const ix = client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        });

        await processInstruction(context, ix);

        // Try again
        const result = await tryProcessInstruction(context, ix);
        expect(result.result).toContain("instruction requires an uninitialized account"); // AlreadyInitialized in our util usually returns this or specific error
        // Actually, initialize_pda_account returns ProgramError::AccountAlreadyInitialized if lamports > 0
    });

    it("Failure: Invalid PDA seeds (wrong authority PDA)", async () => {
        const userSeed = new Uint8Array(32).fill(12);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const owner = Keypair.generate();

        // Use a different seed for the PDA than what's in instruction data
        const wrongAuthPda = (await findAuthorityPda(walletPda, new Uint8Array(32).fill(99)))[0];
        const actualBump = (await findAuthorityPda(walletPda, owner.publicKey.toBytes()))[1];

        const result = await tryProcessInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: wrongAuthPda,
            userSeed,
            authType: 0,
            authBump: actualBump,
            authPubkey: owner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        }));

        expect(result.result).toContain("Provided seeds do not result in a valid address");
    });

    // --- Category 2: SDK Encoding Correctness ---

    it("Encoding: Ed25519 CreateWallet data matches expected binary layout", async () => {
        const userSeed = new Uint8Array(32).fill(13);
        const owner = Keypair.generate();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const [authPda, authBump] = await findAuthorityPda(walletPda, owner.publicKey.toBytes());

        const ix = client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        });

        const data = Buffer.from(ix.data);
        // Layout: [disc(1)][userSeed(32)][authType(1)][authBump(1)][padding(6)][pubkey(32)]
        // Total: 1 + 32 + 1 + 1 + 6 + 32 = 73
        expect(data.length).toBe(73);
        expect(data[0]).toBe(0);                                              // discriminator
        expect(Buffer.from(data.subarray(1, 33))).toEqual(Buffer.from(userSeed)); // userSeed
        expect(data[33]).toBe(0);                                             // authType = Ed25519
        expect(data[34]).toBe(authBump);                                      // bump
        expect(data.subarray(35, 41)).toEqual(Buffer.alloc(6));               // padding
        expect(Buffer.from(data.subarray(41, 73))).toEqual(Buffer.from(owner.publicKey.toBytes()));
    });

    it("Encoding: Secp256r1 CreateWallet data matches expected binary layout", async () => {
        const userSeed = new Uint8Array(32).fill(14);
        const credentialIdHash = Buffer.alloc(32, 0xAA);
        const p256Pubkey = Buffer.alloc(33, 0xBB);
        p256Pubkey[0] = 0x02;

        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        const ix = client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1,
            authBump,
            authPubkey: p256Pubkey,
            credentialHash: credentialIdHash,
        });

        const data = Buffer.from(ix.data);
        // Layout: [disc(1)][userSeed(32)][authType(1)][authBump(1)][padding(6)][credIdHash(32)][pubkey(33)]
        // Total: 1 + 32 + 1 + 1 + 6 + 32 + 33 = 106
        expect(data.length).toBe(106);
        expect(data[0]).toBe(0);                                                 // discriminator
        expect(data[33]).toBe(1);                                                // authType = Secp256r1
        expect(Buffer.from(data.subarray(41, 73))).toEqual(credentialIdHash);    // credential_id_hash
        expect(Buffer.from(data.subarray(73, 106))).toEqual(p256Pubkey);         // pubkey
    });
});
